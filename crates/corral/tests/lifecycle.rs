// From Saddle `crates/corral-core/tests/lifecycle.rs` at commit `a31dea2`, unchanged.
use serde_json::Value;
use std::process::{Command, Output};
struct Lab {
    root: tempfile::TempDir,
    core: std::path::PathBuf,
}
impl Lab {
    fn new() -> Self {
        let root = tempfile::tempdir_in("/tmp").unwrap();
        let core = root.path().join("corral");
        std::fs::copy(env!("CARGO_BIN_EXE_corral"), &core).unwrap();
        Self { root, core }
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(&self.core)
            .args(args)
            .env("CORRAL_HOME", self.root.path().join("pens"))
            .env("HOME", self.root.path())
            .env_remove("CODEX_SANDBOX")
            .output()
            .unwrap()
    }
    fn json(&self, args: &[&str]) -> Value {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "{args:?}: {} {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
}
impl Drop for Lab {
    fn drop(&mut self) {
        let _ = self.run(&["stop", "test/cat", "--timeout", "5"]);
    }
}
#[test]
fn detached_agent_survives_start_client_and_stops_explicitly() {
    let lab = Lab::new();
    let start = lab.json(&["start", "test/cat", "--cwd", "/tmp", "--", "/bin/cat"]);
    assert_eq!(start["name"], "test/cat");
    assert_eq!(start["instance"].as_str().unwrap().len(), 12);
    let status = lab.json(&["status", "test/cat"]);
    assert_eq!(status["instance"], start["instance"]);
    assert_eq!(status["state"], "unknown");
    let duplicate = lab.run(&["start", "test/cat", "--", "/bin/cat"]);
    assert_eq!(duplicate.status.code(), Some(5));
    let stopped = lab.json(&["stop", "test/cat", "--timeout", "5"]);
    assert_eq!(stopped["instance"], start["instance"]);
    assert_eq!(lab.run(&["status", "test/cat"]).status.code(), Some(2));
}
#[test]
fn raw_agent_can_receive_text_and_expose_output_without_false_confirmation() {
    let lab = Lab::new();
    let start = lab.json(&["start", "test/cat", "--cwd", "/tmp", "--", "/bin/cat"]);
    let sent = lab.json(&["send", "test/cat", "hello runtime"]);
    assert_eq!(sent["instance"], start["instance"]);
    assert_eq!(sent["confirmed"], false);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let output = lab.run(&["read", "test/cat"]);
        assert!(output.status.success());
        if String::from_utf8_lossy(&output.stdout).contains("hello runtime") {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "output was not captured"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let list = lab.json(&["ls"]);
    assert_eq!(list["agents"][0]["instance"], start["instance"]);
    assert_eq!(lab.json(&["where", "test/cat"])["cwd"], "/tmp");
}
#[test]
fn terminal_attach_detaches_without_stopping_agent() {
    use std::{
        fs::File,
        io::{Read, Write},
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::process::CommandExt,
        },
        process::Stdio,
        time::{Duration, Instant},
    };
    let lab = Lab::new();
    let start = lab.json(&["start", "test/cat", "--", "/bin/cat"]);
    let mut master = -1;
    let mut slave = -1;
    let mut size = libc::winsize {
        ws_row: 24,
        ws_col: 80,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut size,
            )
        },
        0
    );
    let mut master = unsafe { File::from_raw_fd(master) };
    let slave = unsafe { File::from_raw_fd(slave) };
    unsafe {
        libc::fcntl(master.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC);
        libc::fcntl(master.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK);
    }
    let mut c = Command::new(&lab.core);
    c.args(["attach", "test/cat"])
        .env("CORRAL_HOME", lab.root.path().join("pens"))
        .env_remove("CODEX_SANDBOX")
        .stdin(slave.try_clone().unwrap())
        .stdout(slave.try_clone().unwrap())
        .stderr(Stdio::null());
    unsafe {
        c.pre_exec(|| {
            libc::setsid();
            libc::ioctl(0, libc::TIOCSCTTY as _, 0);
            Ok(())
        });
    }
    let mut child = c.spawn().unwrap();
    drop(c);
    drop(slave);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if lab.json(&["status", "test/cat"])["attached"] == 1 {
            break;
        }
        assert!(
            child.try_wait().unwrap().is_none(),
            "attach exited before connecting"
        );
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    master.write_all(b"attached text\r").unwrap();
    let mut output = Vec::new();
    let mut buf = [0; 4096];
    loop {
        if let Ok(n) = master.read(&mut buf) {
            output.extend_from_slice(&buf[..n]);
        }
        if String::from_utf8_lossy(&output).contains("attached text") {
            break;
        }
        assert!(Instant::now() < deadline, "no attached output");
        std::thread::sleep(Duration::from_millis(20));
    }
    master.write_all(b"\x1d").unwrap();
    loop {
        let _ = master.read(&mut buf);
        if let Some(s) = child.try_wait().unwrap() {
            assert!(s.success());
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    let st = lab.json(&["status", "test/cat"]);
    assert_eq!(st["instance"], start["instance"]);
    assert_eq!(st["attached"], 0);
}
#[test]
fn hook_state_and_reply_confirm_delivery_for_recognized_agent() {
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        time::{Duration, Instant},
    };
    let lab = Lab::new();
    let fake = lab.root.path().join("claude");
    fs::write(&fake,r##"#!/bin/sh
for a in "$@"; do last=$a; done
emit() { printf '%s' "$2" | "$FAKE_CORE" __hook "$1"; }
emit SessionStart "{\"session_id\":\"test-session\",\"cwd\":\"$PWD\",\"transcript_path\":\"fake\"}"
turn() {
  emit UserPromptSubmit "{\"session_id\":\"test-session\",\"prompt\":\"$1\"}"
  printf 'received:%s\n' "$1"
  emit Stop "{\"session_id\":\"test-session\",\"last_assistant_message\":\"reply:$1\",\"background_tasks\":[]}"
}
turn "$last"
while IFS= read -r text; do turn "$text"; done
"##).unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();
    let core = format!("FAKE_CORE={}", lab.core.display());
    let s = lab.json(&[
        "start",
        "test/cat",
        "--cwd",
        "/tmp",
        "--env",
        &core,
        "--label",
        "role=controller",
        "--prompt",
        "initial",
        "--",
        fake.to_str().unwrap(),
    ]);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let out = lab.run(&["reply", "test/cat"]);
        if out.status.success() {
            let v: Value = serde_json::from_slice(&out.stdout).unwrap();
            assert_eq!(v["text"], "reply:initial");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let st = lab.json(&["status", "test/cat"]);
    assert_eq!(st["state"], "idle");
    assert_eq!(st["labels"]["role"], "controller");
    let sent = lab.json(&["send", "test/cat", "followup", "--timeout", "2"]);
    assert_eq!(sent["confirmed"], true);
    assert_eq!(sent["merged_with_draft"], false);
    assert_eq!(sent["instance"], s["instance"]);
    lab.json(&["wait", "test/cat", "--timeout", "3"]);
    assert_eq!(lab.json(&["reply", "test/cat"])["text"], "reply:followup");
}
#[test]
fn deferred_send_is_delivered_by_detached_worker() {
    let lab = Lab::new();
    let s = lab.json(&["start", "test/cat", "--", "/bin/cat"]);
    let p = lab.json(&[
        "send",
        "test/cat",
        "later text",
        "--after",
        "test/cat",
        "--timeout",
        "2",
    ]);
    assert_eq!(p["pending"], true);
    assert_eq!(p["after_instance"], s["instance"]);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
    loop {
        let output = lab.run(&["read", "test/cat"]);
        if String::from_utf8_lossy(&output.stdout).contains("later text") {
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
#[test]
fn skill_install_requires_consent_and_preserves_unowned_removal() {
    let lab = Lab::new();
    let dry = lab.json(&["install-skills", "--target", "codex", "--dry-run"]);
    assert_eq!(dry["written"], false);
    assert_eq!(dry["items"][0]["status"], "create");
    let path = lab.root.path().join(".agents/skills/corral/SKILL.md");
    assert!(!path.exists());
    let refused = lab.run(&["install-skills", "--target", "codex"]);
    assert_eq!(refused.status.code(), Some(1));
    let error: Value = serde_json::from_slice(&refused.stdout).unwrap();
    assert_eq!(error["error"], "confirmation_required");
    assert_eq!(
        lab.json(&["install-skills", "--target", "codex", "--yes"])["written"],
        true
    );
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("corral-skill:")
    );
    std::fs::write(&path, "user owned skill").unwrap();
    let removed = lab.json(&["install-skills", "--target", "codex", "--remove", "--yes"]);
    assert_eq!(removed["written"], false);
    assert_eq!(removed["items"][0]["status"], "foreign");
    assert_eq!(std::fs::read_to_string(path).unwrap(), "user owned skill");
}

#[test]
fn attach_checks_missing_name_before_tty_and_wait_can_be_cancelled() {
    use std::{
        fs::File,
        io::Read,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::process::CommandExt,
        },
        time::{Duration, Instant},
    };
    let lab = Lab::new();
    assert_eq!(lab.run(&["attach", "missing"]).status.code(), Some(2));
    let mut master = -1;
    let mut slave = -1;
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        0
    );
    let mut master = unsafe { File::from_raw_fd(master) };
    let slave = unsafe { File::from_raw_fd(slave) };
    unsafe {
        libc::fcntl(master.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC);
        libc::fcntl(master.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK);
    }
    let mut command = Command::new(&lab.core);
    command
        .args(["attach", "missing", "--wait"])
        .env("CORRAL_HOME", lab.root.path().join("pens"))
        .env_remove("CODEX_SANDBOX")
        .stdin(slave.try_clone().unwrap())
        .stdout(slave.try_clone().unwrap())
        .stderr(slave.try_clone().unwrap());
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            libc::ioctl(0, libc::TIOCSCTTY as _, 0);
            Ok(())
        });
    }
    let mut child = command.spawn().unwrap();
    drop(command);
    drop(slave);
    let until = Instant::now() + Duration::from_secs(3);
    let mut bytes = Vec::new();
    let mut buf = [0; 4096];
    loop {
        if let Ok(n) = master.read(&mut buf) {
            bytes.extend_from_slice(&buf[..n]);
        }
        if String::from_utf8_lossy(&bytes).contains("waiting for missing") {
            break;
        }
        if Instant::now() >= until {
            let _ = child.kill();
            let _ = child.wait();
            panic!("waiting hint missing")
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    unsafe {
        libc::kill(child.id() as i32, libc::SIGINT);
    }
    loop {
        let _ = master.read(&mut buf);
        if let Some(status) = child.try_wait().unwrap() {
            assert_eq!(status.code(), Some(0));
            break;
        }
        if Instant::now() >= until {
            let _ = child.kill();
            let _ = child.wait();
            panic!("wait did not cancel")
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn public_subcommands_expose_help_without_creating_state() {
    let lab = Lab::new();
    for command in [
        "start",
        "send",
        "keys",
        "status",
        "wait",
        "reply",
        "where",
        "ls",
        "read",
        "attach",
        "stop",
        "upgrade",
        "recover",
        "after",
        "guide",
        "install-skills",
    ] {
        let out = lab.run(&[command, "--help"]);
        assert!(
            out.status.success(),
            "{command}: {}",
            String::from_utf8_lossy(&out.stdout)
        );
        assert!(String::from_utf8_lossy(&out.stdout).contains(&format!("corral {command}")));
    }
    assert!(!lab.root.path().join("pens").exists());
}
