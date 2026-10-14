//! `corral integration status`: what the adapter registry supports and how this machine stands,
//! for front ends to show. Read-only: it writes nothing and only runs each agent's `--version`.
use crate::{Result, agents, skills};
use serde_json::{Value, json};
use std::{
    io::Read,
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

/// How long one agent's `--version` may take before it and its process group are killed.
const VERSION_TIMEOUT: Duration = Duration::from_secs(3);

pub fn status() -> Result<Value> {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let found: Vec<_> = agents::all().map(|a| (a, find(a.kind()))).collect();
    let versions: Vec<Option<String>> = std::thread::scope(|scope| {
        let running: Vec<_> = found
            .iter()
            .map(|(_, path)| scope.spawn(|| path.as_deref().and_then(version)))
            .collect();
        running
            .into_iter()
            .map(|h| h.join().unwrap_or(None))
            .collect()
    });
    let mut list = Vec::new();
    for ((adapter, path), version) in found.iter().zip(versions) {
        let mut places = Vec::new();
        for target in skills::TARGETS {
            if adapter.skill_targets().contains(&target) {
                let (file, status) = skills::plan(target, &home, None, false)?;
                places.push(json!({"target":target,"path":file,"status":status}));
            }
        }
        list.push(json!({"kind":adapter.kind(),"path":path,"version":version,"hooks":"launch","skills":places}));
    }
    Ok(json!({"ok":true,"agents":list}))
}

/// The first executable file named `program` in an absolute directory on PATH.
fn find(program: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(program))
        .find(|p| {
            p.metadata()
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}

/// The first non-empty line `program --version` prints, if it succeeds within the timeout.
fn version(program: &Path) -> Option<String> {
    let mut child = Command::new(program)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .ok()?;
    let group = child.id() as libc::pid_t;
    let mut stdout = child.stdout.take()?;
    let (tx, rx) = mpsc::channel();
    // A child the program leaves behind may hold the pipe open, so read on a thread of its own.
    std::thread::spawn(move || {
        let mut out = Vec::new();
        let _ = stdout.read_to_end(&mut out);
        let _ = tx.send(out);
    });
    let deadline = Instant::now() + VERSION_TIMEOUT;
    let out = rx.recv_timeout(VERSION_TIMEOUT).ok();
    let mut status = None;
    while out.is_some() && Instant::now() < deadline {
        if let Ok(Some(s)) = child.try_wait() {
            status = Some(s);
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if status.is_none() {
        // Only the group this call started: the program and whatever it spawned.
        unsafe { libc::kill(-group, libc::SIGKILL) };
        let _ = child.wait();
    }
    if !status?.success() {
        return None;
    }
    String::from_utf8_lossy(&out?)
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_owned)
}
