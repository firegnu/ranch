//! Pause and resume: the agent's whole tree stops and continues; other commands respect it.
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::PathBuf,
    process::{Command, Output},
    time::{Duration, Instant},
};

struct Lab {
    root: tempfile::TempDir,
    core: PathBuf,
    names: std::cell::RefCell<Vec<String>>,
    strays: std::cell::RefCell<Vec<i32>>,
}
impl Lab {
    fn new() -> Self {
        let root = tempfile::tempdir_in("/tmp").unwrap();
        let core = root.path().join("v1/corral");
        fs::create_dir_all(core.parent().unwrap()).unwrap();
        fs::copy(env!("CARGO_BIN_EXE_corral"), &core).unwrap();
        Self {
            root,
            core,
            names: Default::default(),
            strays: Default::default(),
        }
    }
    fn out(&self, args: &[&str]) -> Output {
        Command::new(&self.core)
            .args(args)
            .env("HOME", self.root.path())
            .env("CORRAL_HOME", self.root.path().join("pens"))
            .env("SHELL", "/bin/sh")
            .env_remove("CODEX_SANDBOX")
            .output()
            .unwrap()
    }
    fn json(&self, args: &[&str]) -> Value {
        let o = self.out(args);
        assert!(
            o.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&o.stdout)
        );
        serde_json::from_slice(&o.stdout).unwrap()
    }
    fn script(&self, file: &str, body: &str) -> String {
        let path = self.root.path().join(file);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path.to_str().unwrap().to_owned()
    }
    fn start(&self, name: &str, program: &str) -> Value {
        self.names.borrow_mut().push(name.to_owned());
        self.json(&["start", name, "--cwd", "/tmp", "--", program])
    }
    /// An agent kind with hooks, so it has idle/working states.
    fn known(&self, name: &str) {
        let claude = self.script("claude", "exec /bin/cat");
        self.start(name, &claude);
        self.event(
            name,
            "SessionStart",
            json!({"cwd":"/tmp","has_transcript":true}),
        );
    }
    fn event(&self, name: &str, ev: &str, extra: Value) {
        let instance = self.json(&["status", name])["instance"].clone();
        let mut e = json!({"v":1,"inst":instance,"session_id":"main","t":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64(),"ev":ev});
        e.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let path = self.root.path().join("pens").join(name).join("events");
        writeln!(
            fs::OpenOptions::new().append(true).open(path).unwrap(),
            "{e}"
        )
        .unwrap();
    }
    fn read(&self, name: &str) -> String {
        String::from_utf8_lossy(&self.out(&["read", name]).stdout).into_owned()
    }
    /// Starts an agent that prints the pid of a child it put in its own session.
    fn with_stray(&self, name: &str) -> (i32, i32) {
        let agent = self.script(
            "agent",
            "/usr/bin/perl -MPOSIX -e 'POSIX::setsid(); $|=1; print \"stray:$$:\\n\"; sleep 60' &\nexec /bin/cat",
        );
        self.start(name, &agent);
        let mut stray = None;
        eventually(|| {
            stray = self
                .read(name)
                .split("stray:")
                .nth(1)
                .and_then(|s| s.split(':').next()?.parse().ok());
            stray.is_some()
        });
        let stray = stray.unwrap();
        self.strays.borrow_mut().push(stray);
        let agent = self.json(&["status", name])["agent_pid"].as_i64().unwrap() as i32;
        (agent, stray)
    }
}
impl Drop for Lab {
    fn drop(&mut self) {
        for name in self.names.borrow().iter() {
            let _ = self.out(&["stop", name, "--timeout", "5"]);
        }
        for &pid in self.strays.borrow().iter() {
            unsafe {
                libc::kill(pid, libc::SIGCONT);
                libc::kill(pid, libc::SIGKILL);
            }
        }
    }
}
fn eventually(mut check: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(5);
    while !check() {
        assert!(Instant::now() < end, "condition not reached");
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn stopped(pid: i32) -> bool {
    let out = Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().starts_with('T')
}

#[test]
fn pause_stops_the_whole_tree_and_resume_continues_it() {
    let lab = Lab::new();
    let (agent, stray) = lab.with_stray("test/a");
    let before = lab.json(&["status", "test/a"]);
    assert_eq!(before["paused"], false);
    assert!(before["paused_at"].is_null());

    let paused = lab.json(&["pause", "test/a"]);
    assert_eq!(paused["paused"], true);
    eventually(|| stopped(agent) && stopped(stray));
    let status = lab.json(&["status", "test/a"]);
    assert_eq!(status["paused"], true);
    assert!(status["paused_at"].as_f64().is_some());
    assert_eq!(status["instance"], before["instance"]);
    let listed = lab.json(&["ls"]);
    assert_eq!(listed["agents"][0]["paused"], true);

    // Pausing again changes nothing.
    let again = lab.json(&["pause", "test/a"]);
    assert_eq!(again["paused_at"], status["paused_at"]);

    let resumed = lab.json(&["resume", "test/a"]);
    assert_eq!(resumed["paused"], false);
    eventually(|| !stopped(agent) && !stopped(stray));
    let status = lab.json(&["status", "test/a"]);
    assert_eq!(status["paused"], false);
    assert_eq!(lab.json(&["ls"])["agents"][0]["paused"], false);
    // Resuming what is not paused is fine too.
    assert_eq!(lab.json(&["resume", "test/a"])["paused"], false);
    lab.json(&["send", "test/a", "awake again"]);
    eventually(|| lab.read("test/a").contains("awake again"));
}

#[test]
fn paused_agent_refuses_send_and_keys_and_drops_typing() {
    let lab = Lab::new();
    lab.start("test/a", "/bin/cat");
    lab.json(&["pause", "test/a"]);

    let send = lab.out(&["send", "test/a", "while-paused"]);
    assert_eq!(send.status.code(), Some(10));
    let reply: Value = serde_json::from_slice(&send.stdout).unwrap();
    assert_eq!(reply["error"], "paused");
    let keys = lab.out(&["keys", "test/a", "text:keys-paused"]);
    assert_eq!(keys.status.code(), Some(10));

    let mut s = UnixStream::connect(lab.root.path().join("pens/test/a/sock")).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    writeln!(
        s,
        "{}",
        json!({"proto":1,"op":"attach","rows":24,"cols":80})
    )
    .unwrap();
    let mut b = [0];
    loop {
        s.read_exact(&mut b).unwrap();
        if b[0] == b'\n' {
            break;
        }
    }
    let typed = b"typed-paused\r";
    s.write_all(b"i").unwrap();
    s.write_all(&(typed.len() as u32).to_be_bytes()).unwrap();
    s.write_all(typed).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    lab.json(&["resume", "test/a"]);
    std::thread::sleep(Duration::from_millis(300));
    let seen = lab.read("test/a");
    for marker in ["while-paused", "keys-paused", "typed-paused"] {
        assert!(!seen.contains(marker), "{marker} reached the agent: {seen}");
    }
    drop(s);
    lab.json(&["send", "test/a", "after-resume"]);
    eventually(|| lab.read("test/a").contains("after-resume"));
}

#[test]
fn wait_neither_finishes_nor_goes_quiet_while_paused() {
    let lab = Lab::new();
    lab.known("test/a");
    lab.json(&["pause", "test/a"]);
    // Idle, but paused: not a finished turn.
    let idle = lab.out(&["wait", "test/a", "--timeout", "1"]);
    assert_eq!(
        idle.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&idle.stdout)
    );
    lab.json(&["resume", "test/a"]);
    assert_eq!(
        lab.json(&["wait", "test/a", "--timeout", "3"])["result"],
        "idle"
    );

    lab.event("test/a", "UserPromptSubmit", json!({"prompt":"work"}));
    lab.json(&["pause", "test/a"]);
    let quiet = lab.out(&["wait", "test/a", "--timeout", "1.5", "--quiet", "0.5"]);
    assert_eq!(
        quiet.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&quiet.stdout)
    );
    // The quiet clock starts again at resume.
    lab.json(&["resume", "test/a"]);
    let t0 = Instant::now();
    let quiet = lab.json(&["wait", "test/a", "--timeout", "5", "--quiet", "1"]);
    assert_eq!(quiet["result"], "stopped-quiet");
    assert!(
        t0.elapsed() >= Duration::from_millis(800),
        "{:?}",
        t0.elapsed()
    );
}

#[test]
fn reminders_wait_for_a_paused_agent_on_either_side() {
    let lab = Lab::new();
    lab.known("test/a");
    lab.start("test/b", "/bin/cat");

    // The agent being waited on is idle but paused: not finished.
    lab.json(&["pause", "test/a"]);
    let first = lab.json(&[
        "send",
        "test/b",
        "first-note",
        "--after",
        "test/a",
        "--timeout",
        "10",
    ]);
    std::thread::sleep(Duration::from_millis(1500));
    assert!(!lab.read("test/b").contains("first-note"));
    lab.json(&["resume", "test/a"]);
    eventually(|| lab.read("test/b").contains("first-note"));
    let id = first["request_id"].as_str().unwrap().to_owned();
    eventually(|| {
        lab.json(&["after", "test/b", "--request-id", &id])["records"][0]["phase"] == "sent"
    });

    // The recipient is paused: the note waits for it instead of failing.
    lab.json(&["pause", "test/b"]);
    let second = lab.json(&[
        "send",
        "test/b",
        "second-note",
        "--after",
        "test/a",
        "--timeout",
        "10",
    ]);
    std::thread::sleep(Duration::from_millis(1500));
    let id = second["request_id"].as_str().unwrap().to_owned();
    let record = lab.json(&["after", "test/b", "--request-id", &id])["records"][0].clone();
    assert_eq!(record["phase"], "waiting", "{record}");
    lab.json(&["resume", "test/b"]);
    eventually(|| lab.read("test/b").contains("second-note"));
}

#[test]
fn stop_continues_a_paused_agent_first() {
    let lab = Lab::new();
    let (_agent, stray) = lab.with_stray("test/a");
    lab.json(&["pause", "test/a"]);
    eventually(|| stopped(stray));
    let stop = lab.json(&["stop", "test/a", "--timeout", "5"]);
    assert_eq!(stop["stopped_by"], "SIGHUP", "{stop}");
    // Nothing it froze is left frozen.
    eventually(|| !stopped(stray));
}

#[test]
fn pause_survives_an_upgrade() {
    let lab = Lab::new();
    let (agent, stray) = lab.with_stray("test/a");
    lab.json(&["pause", "test/a"]);
    let new = lab.root.path().join("v2/corral");
    fs::create_dir_all(new.parent().unwrap()).unwrap();
    fs::copy(env!("CARGO_BIN_EXE_corral"), &new).unwrap();
    let up = lab.json(&["upgrade", "test/a", "--exe", new.to_str().unwrap()]);
    assert_eq!(up["result"], "complete", "{up}");
    let status = lab.json(&["status", "test/a"]);
    assert_eq!(status["exe"], json!(fs::canonicalize(&new).unwrap()));
    assert_eq!(status["paused"], true);
    assert!(stopped(agent) && stopped(stray));
    lab.json(&["resume", "test/a"]);
    eventually(|| !stopped(agent) && !stopped(stray));
}

#[test]
fn paused_agent_is_not_handed_to_a_pen_that_cannot_keep_it_paused() {
    let lab = Lab::new();
    lab.start("test/a", "/bin/cat");
    lab.json(&["pause", "test/a"]);
    // An image that speaks the snapshot schema but knows nothing of pausing.
    let old = lab.script(
        "old-corral",
        "if [ \"$1\" = __pen-probe ]; then echo '{\"ok\":true,\"schema\":1}'; exit 0; fi\nexit 1",
    );
    let up = lab.out(&["upgrade", "test/a", "--exe", &old]);
    let reply: Value = serde_json::from_slice(&up.stdout).unwrap();
    assert_eq!(reply["result"], "failed", "{reply}");
    assert!(reply.to_string().contains("paused"), "{reply}");
    let status = lab.json(&["status", "test/a"]);
    assert_eq!(status["paused"], true);
    assert_eq!(status["exe"], json!(fs::canonicalize(&lab.core).unwrap()));
}

#[test]
fn pause_reaches_a_process_whose_parent_already_exited() {
    // A child in its own group forks a grandchild and exits: init adopts the grandchild,
    // which is no longer below the agent but is still in its session.
    let lab = Lab::new();
    let agent = lab.script(
        "agent",
        "/usr/bin/perl -e 'setpgrp(0,0); if (fork() == 0) { $|=1; print \"orphan:$$:\\n\"; sleep 60; exit } exit' &\nexec /bin/cat",
    );
    lab.start("test/a", &agent);
    let mut orphan = None;
    eventually(|| {
        orphan = lab
            .read("test/a")
            .split("orphan:")
            .nth(1)
            .and_then(|s| s.split(':').next()?.parse::<i32>().ok());
        orphan.is_some()
    });
    let orphan = orphan.unwrap();
    lab.strays.borrow_mut().push(orphan);
    let parent = Command::new("ps")
        .args(["-o", "ppid=", "-p", &orphan.to_string()])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&parent.stdout).trim(), "1");
    lab.json(&["pause", "test/a"]);
    eventually(|| stopped(orphan));
    lab.json(&["resume", "test/a"]);
    eventually(|| !stopped(orphan));
}

#[test]
fn a_paused_agent_without_hooks_is_not_finished_either() {
    let lab = Lab::new();
    lab.start("test/a", "/bin/cat");
    lab.start("test/b", "/bin/cat");
    lab.json(&["pause", "test/a"]);
    let wait = lab.out(&["wait", "test/a", "--timeout", "1"]);
    assert_eq!(
        wait.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&wait.stdout)
    );
    lab.json(&[
        "send",
        "test/b",
        "raw-note",
        "--after",
        "test/a",
        "--timeout",
        "10",
    ]);
    std::thread::sleep(Duration::from_millis(1500));
    assert!(!lab.read("test/b").contains("raw-note"));
    lab.json(&["resume", "test/a"]);
    assert_eq!(
        lab.json(&["wait", "test/a", "--timeout", "1"])["result"],
        "unknown"
    );
    eventually(|| lab.read("test/b").contains("raw-note"));
}

#[test]
fn a_message_accepted_before_a_pause_is_not_reported_lost() {
    let lab = Lab::new();
    lab.known("test/a");
    let name = "test/a".to_owned();
    let core = lab.core.clone();
    let root = lab.root.path().to_owned();
    // The synthetic agent never confirms input, so without a pause this send ends in 3.
    let pauser = std::thread::spawn(move || {
        let run = |op: &str| {
            Command::new(&core)
                .args([op, &name])
                .env("HOME", &root)
                .env("CORRAL_HOME", root.join("pens"))
                .env_remove("CODEX_SANDBOX")
                .output()
                .unwrap()
        };
        std::thread::sleep(Duration::from_millis(600));
        assert!(run("pause").status.success());
        std::thread::sleep(Duration::from_millis(400));
        assert!(run("resume").status.success());
    });
    let sent = lab.json(&["send", "test/a", "held-up", "--timeout", "2"]);
    pauser.join().unwrap();
    assert_eq!(sent["confirmed"], false, "{sent}");
    assert_eq!(sent["paused"], true, "{sent}");
}

#[test]
fn hold_continues_what_it_froze_once_the_agent_is_gone() {
    let lab = Lab::new();
    let (agent, stray) = lab.with_stray("test/a");
    lab.json(&["pause", "test/a"]);
    // A metadata mismatch makes the new image refuse the snapshot, and the pen falls to Hold.
    let meta = lab.root.path().join("pens/test/a/meta.json");
    let mut m: Value = serde_json::from_slice(&fs::read(&meta).unwrap()).unwrap();
    m["instance"] = json!("mismatched");
    fs::write(&meta, m.to_string()).unwrap();
    let new = lab.root.path().join("v2/corral");
    fs::create_dir_all(new.parent().unwrap()).unwrap();
    fs::copy(env!("CARGO_BIN_EXE_corral"), &new).unwrap();
    // The upgrade command itself takes a while to give up on Hold; it is not waited for.
    let mut upgrade = Command::new(&lab.core)
        .args(["upgrade", "test/a", "--exe", new.to_str().unwrap()])
        .env("HOME", lab.root.path())
        .env("CORRAL_HOME", lab.root.path().join("pens"))
        .env_remove("CODEX_SANDBOX")
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    eventually(|| {
        let st = lab.out(&["status", "test/a"]);
        let st: Value = serde_json::from_slice(&st.stdout).unwrap_or_default();
        matches!(
            st["upgrade"]["state"].as_str(),
            Some("hold" | "hold_unprotected")
        )
    });
    assert!(stopped(stray));
    unsafe {
        libc::kill(agent, libc::SIGKILL);
    }
    eventually(|| !stopped(stray));
    // Hold keeps the pen, and its standby child keeps the socket: end both by PID.
    let st: Value = serde_json::from_slice(&lab.out(&["status", "test/a"]).stdout).unwrap();
    let pen = st["pen_pid"].as_i64().unwrap().to_string();
    let children = Command::new("pgrep").args(["-P", &pen]).output().unwrap();
    for pid in String::from_utf8_lossy(&children.stdout)
        .split_whitespace()
        .chain([pen.as_str()])
    {
        unsafe {
            libc::kill(pid.parse().unwrap(), libc::SIGKILL);
        }
    }
    let _ = upgrade.kill();
    let _ = upgrade.wait();
}
