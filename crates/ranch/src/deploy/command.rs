use std::{
    io::{self, Read},
    os::unix::process::CommandExt,
    path::Path,
    process::{Command, Output, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

/// Starts target-version programs or /bin/launchctl. Bound process lifetime and pipe reads;
/// On failure, kill/reap its group only while the leader has not been reaped.
pub(super) fn run(program: &Path, args: &[String], timeout: Duration) -> Result<Output, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|e| format!("{}: {e}", program.display()))?;
    let group = child.id() as libc::pid_t;
    let (tx, rx) = mpsc::channel();
    fn read(mut pipe: impl Read, tx: mpsc::Sender<(bool, io::Result<Vec<u8>>)>, stderr: bool) {
        let mut bytes = Vec::new();
        let result = pipe.read_to_end(&mut bytes).map(|_| bytes);
        let _ = tx.send((stderr, result));
    }
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let err_tx = tx.clone();
    std::thread::spawn(move || read(stdout, tx, false));
    std::thread::spawn(move || read(stderr, err_tx, true));
    let deadline = Instant::now() + timeout;
    let mut status = None;
    let (mut stdout, mut stderr) = (None, None);
    let result = loop {
        for (is_err, bytes) in rx.try_iter() {
            if is_err {
                stderr = Some(bytes);
            } else {
                stdout = Some(bytes);
            }
        }
        if status.is_none() {
            match child.try_wait() {
                Ok(value) => status = value,
                Err(e) => break Err(e.to_string()),
            }
        }
        if let Some(status) = status
            && stdout.is_some()
            && stderr.is_some()
        {
            break match (stdout.unwrap(), stderr.unwrap()) {
                (Ok(stdout), Ok(stderr)) => Ok(Output {
                    status,
                    stdout,
                    stderr,
                }),
                (Err(e), _) | (_, Err(e)) => Err(e.to_string()),
            };
        }
        if Instant::now() >= deadline {
            break Err(format!("超过 {} 秒", timeout.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    if result.is_err() && status.is_none() {
        // An unreaped leader still reserves its PID. After reaping it, the group
        // may have disappeared and its number may belong to an unrelated process.
        unsafe {
            libc::kill(-group, libc::SIGKILL);
        }
        let _ = child.wait();
    }
    result.map_err(|e| format!("{} {}: {e}", program.display(), args.join(" ")))
}
