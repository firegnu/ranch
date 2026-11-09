//! Explicit steering through the CLI and pen, with isolated synthetic agents.
mod common;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    process::{Command, Output},
    time::{Duration, Instant},
};

struct Lab {
    root: tempfile::TempDir,
    core: PathBuf,
}
impl Lab {
    fn new() -> Self {
        let root = tempfile::tempdir_in("/tmp").unwrap();
        let core = root.path().join("corral");
        common::copy_exe(env!("CARGO_BIN_EXE_corral"), &core);
        Self { root, core }
    }
    fn cmd(&self) -> Command {
        let mut cmd = Command::new(&self.core);
        cmd.env("HOME", self.root.path())
            .env("CORRAL_HOME", self.root.path().join("pens"))
            .env("SHELL", "/bin/sh")
            .env_remove("CODEX_SANDBOX")
            .env_remove("FENCE_RUN")
            .env_remove("CORRAL_NAME")
            .env_remove("CORRAL_INSTANCE");
        cmd
    }
    fn out(&self, args: &[&str]) -> Output {
        self.cmd().args(args).output().unwrap()
    }
    fn json(&self, args: &[&str]) -> Value {
        let out = self.out(args);
        assert!(out.status.success(), "{args:?}: {out:?}");
        serde_json::from_slice(&out.stdout).unwrap()
    }
    fn start(&self, kind: &str, mode: &str) {
        let agent = self.root.path().join(kind);
        fs::write(
            &agent,
            format!(
                "#!/bin/sh\nexec '{}' --exact steer_synthetic_agent --ignored --nocapture\n",
                std::env::current_exe().unwrap().display()
            ),
        )
        .unwrap();
        fs::set_permissions(&agent, fs::Permissions::from_mode(0o700)).unwrap();
        self.json(&[
            "start",
            "lab/main",
            "--cwd",
            self.root.path().to_str().unwrap(),
            "--env",
            &format!("R57B_MODE={mode}"),
            "--",
            agent.to_str().unwrap(),
        ]);
        self.until(|| self.read().contains("READY"));
    }
    fn read(&self) -> String {
        String::from_utf8(self.out(&["read", "lab/main"]).stdout).unwrap()
    }
    fn until(&self, mut check: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !check() {
            assert!(Instant::now() < deadline, "{}", self.read());
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn reject(&self, args: &[&str], code: i32, error: &str) -> Value {
        let out = self.out(args);
        let value: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(out.status.code(), Some(code), "{args:?}: {value}");
        assert_eq!(value["error"], error, "{value}");
        value
    }
    fn human_input(&self) -> UnixStream {
        use std::io::BufRead;
        let mut stream = UnixStream::connect(self.root.path().join("pens/lab/main/sock")).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        writeln!(stream, "{}", json!({"op":"attach","rows":40,"cols":120})).unwrap();
        let mut line = String::new();
        std::io::BufReader::new(&stream)
            .read_line(&mut line)
            .unwrap();
        assert_eq!(serde_json::from_str::<Value>(&line).unwrap()["ok"], true);
        stream.write_all(b"i\0\0\0\x01h").unwrap();
        self.until(|| self.json(&["status", "lab/main"])["last_human_input"].is_number());
        stream
    }
}
impl Drop for Lab {
    fn drop(&mut self) {
        let _ = self.out(&["stop", "lab/main", "--timeout", "5"]);
    }
}

fn event(ev: &str, extra: Value) {
    let mut value = json!({
        "v":1, "inst":std::env::var("CORRAL_INSTANCE").unwrap(),
        "session_id":"main", "ev":ev,
        "t":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64(),
    });
    value
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(std::env::var("CORRAL_EVENTS").unwrap())
        .unwrap();
    writeln!(file, "{value}").unwrap();
}

#[test]
#[ignore = "synthetic agent launched by this test binary"]
fn steer_synthetic_agent() {
    let Ok(mode) = std::env::var("R57B_MODE") else {
        return;
    };
    unsafe {
        let mut term = std::mem::zeroed();
        assert_eq!(libc::tcgetattr(0, &mut term), 0);
        libc::cfmakeraw(&mut term);
        assert_eq!(libc::tcsetattr(0, libc::TCSANOW, &term), 0);
    }
    if mode != "starting" {
        event(
            "SessionStart",
            json!({"cwd":std::env::current_dir().unwrap(), "has_transcript":true}),
        );
        if mode != "idle" {
            event("UserPromptSubmit", json!({"prompt":"base task"}));
        }
        if mode == "blocked" {
            event("PermissionRequest", json!({"tool_name":"AskUserQuestion"}));
        }
    }
    if mode != "no_paste" {
        print!("\x1b[?2004h");
    }
    println!("READY");
    std::io::stdout().flush().unwrap();
    let mut bytes = Vec::new();
    loop {
        let mut byte = [0];
        if std::io::stdin().read_exact(&mut byte).is_err() {
            break;
        }
        if byte[0] == 3 {
            break;
        }
        bytes.push(byte[0]);
        println!("RX:{:02x}", byte[0]);
        if bytes.ends_with(b"\x1b[201~") {
            if mode == "block_on_paste" {
                event("PermissionRequest", json!({"tool_name":"AskUserQuestion"}));
            } else if mode == "idle_on_paste" {
                event("Stop", json!({"last_assistant_message":"done"}));
            }
        }
        if byte[0] == b'\r' {
            let text = String::from_utf8_lossy(&bytes);
            let prompt = text
                .trim_end_matches('\r')
                .replace("\x1b[200~", "")
                .replace("\x1b[201~", "");
            if mode != "no_confirmation" {
                event("UserPromptSubmit", json!({"prompt":prompt}));
            }
            bytes.clear();
        }
        std::io::stdout().flush().unwrap();
    }
}

#[test]
fn steer_working_uses_the_adapters_enter_and_confirms_input() {
    for kind in ["claude", "codex", "omp"] {
        let lab = Lab::new();
        lab.start(kind, "working");
        assert_eq!(lab.json(&["status", "lab/main"])["state"], "working");
        let text = "修正第一行\n第二行（甲）[乙]";
        let sent = lab.json(&["send", "lab/main", text, "--steer", "--timeout", "3"]);
        assert_eq!(sent["confirmed"], true, "{kind}: {sent}");
        assert_eq!(sent["merged_with_draft"], false);
        let output = lab.read();
        assert_eq!(output.matches("RX:0d").count(), 1, "{output}");
        assert!(!output.contains("RX:13"), "{output}");
        let status = lab.json(&["status", "lab/main"]);
        assert_eq!(status["last_input_source"], "send");
    }
}

#[test]
fn steer_keeps_the_draft_without_enter_when_state_changes_after_paste() {
    for mode in ["block_on_paste", "idle_on_paste"] {
        let lab = Lab::new();
        lab.start("claude", mode);
        let out = lab.out(&[
            "send",
            "lab/main",
            "draft stays here",
            "--steer",
            "--force",
            "--timeout",
            "1",
            "--request-id",
            "draft-test",
        ]);
        let result: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(out.status.code(), Some(7), "{result}");
        assert_eq!(result["error"], "draft_not_submitted");
        assert_eq!(result["written"], true);
        assert_eq!(result["submitted"], false);
        assert_eq!(result["confirmed"], false);
        assert_eq!(result["request_id"], "draft-test");
        assert!(!lab.read().contains("RX:0d"));
        let status = lab.json(&["status", "lab/main"]);
        assert_eq!(status["recent_sends"][0]["state"], "draft");
        assert_eq!(status["recent_sends"][0]["submitted"], false);
    }
}

#[test]
fn steer_idle_delivers_even_for_an_adapter_without_working_support() {
    for kind in ["claude", "pi"] {
        let lab = Lab::new();
        lab.start(kind, "idle");
        assert_eq!(lab.json(&["status", "lab/main"])["state"], "idle");
        let sent = lab.json(&[
            "send",
            "lab/main",
            "ordinary task",
            "--steer",
            "--timeout",
            "3",
        ]);
        assert_eq!(sent["confirmed"], true);
        assert_eq!(lab.read().matches("RX:0d").count(), 1);
    }
}

#[test]
fn steer_refuses_blocked_starting_paused_and_unsupported_without_writing() {
    for (kind, mode, code, error) in [
        ("claude", "blocked", 7, "not_idle"),
        ("claude", "starting", 7, "not_idle"),
        ("claude", "paused", 10, "paused"),
        ("pi", "working", 9, "unsupported_steer"),
        ("kimi", "working", 9, "unsupported_steer"),
        ("amp", "working", 9, "unsupported_steer"),
        ("agy", "working", 9, "unsupported_steer"),
        ("canter", "working", 9, "unsupported_steer"),
    ] {
        let lab = Lab::new();
        lab.start(kind, mode);
        if mode == "paused" {
            lab.json(&["pause", "lab/main"]);
        }
        for force in [false, true] {
            let mut args = vec!["send", "lab/main", "must not arrive", "--steer"];
            if force {
                args.push("--force");
            }
            lab.reject(&args, code, error);
        }
        assert!(!lab.read().contains("RX:"));
    }
}

#[test]
fn steer_human_input_still_blocks_and_force_only_skips_that_check() {
    let lab = Lab::new();
    lab.start("claude", "working");
    let _attached = lab.human_input();
    lab.reject(&["send", "lab/main", "steer", "--steer"], 8, "human_active");
    let sent = lab.json(&[
        "send",
        "lab/main",
        "steer",
        "--steer",
        "--force",
        "--timeout",
        "3",
    ]);
    assert_eq!(sent["confirmed"], true);
    assert_eq!(sent["merged_with_draft"], true);
    assert_eq!(lab.read().matches("RX:0d").count(), 1);
}

#[test]
fn steer_does_not_resend_when_input_is_unconfirmed() {
    let lab = Lab::new();
    lab.start("claude", "no_confirmation");
    let value = lab.reject(
        &[
            "send",
            "lab/main",
            "only once",
            "--steer",
            "--timeout",
            "0.2",
        ],
        3,
        "not_delivered",
    );
    assert!(value["request_id"].is_string());
    assert_eq!(lab.read().matches("RX:0d").count(), 1);
}

#[test]
fn steer_cannot_be_combined_with_after() {
    let lab = Lab::new();
    lab.reject(
        &[
            "send",
            "lab/main",
            "x",
            "--steer",
            "--after",
            "lab/other",
            "--timeout",
            "1",
        ],
        1,
        "usage",
    );
}

#[test]
fn steer_requires_an_explicit_pen_capability_and_never_falls_back_after_bad_op() {
    use std::io::BufRead;
    for advertised in [false, true] {
        let lab = Lab::new();
        let dir = lab.root.path().join("pens/lab/main");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("meta.json"), r#"{"kind":"cat"}"#).unwrap();
        let listener = UnixListener::bind(dir.join("sock")).unwrap();
        listener.set_nonblocking(true).unwrap();
        std::thread::scope(|scope| {
            let server = scope.spawn(|| {
                let deadline = Instant::now() + Duration::from_secs(5);
                for op in if advertised { vec!["status", "steer"] } else { vec!["status"] } {
                    let mut socket = loop {
                        match listener.accept() {
                            Ok((socket, _)) => break socket,
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                assert!(Instant::now() < deadline);
                                std::thread::sleep(Duration::from_millis(10));
                            }
                            Err(e) => panic!("{e}"),
                        }
                    };
                    socket.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                    let mut line = String::new();
                    std::io::BufReader::new(&socket).read_line(&mut line).unwrap();
                    assert_eq!(serde_json::from_str::<Value>(&line).unwrap()["op"], op);
                    let reply = if op == "status" {
                        json!({"ok":true,"proto":1,"instance":"old","capabilities":if advertised { json!({"steer":1}) } else { json!({}) }})
                    } else {
                        json!({"ok":false,"proto":1,"error":"bad_op"})
                    };
                    writeln!(socket, "{reply}").unwrap();
                }
            });
            lab.reject(
                &["send", "lab/main", "must not arrive", "--steer"],
                9,
                "unsupported",
            );
            server.join().unwrap();
        });
        assert!(matches!(listener.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock));
        drop(listener);
    }
}

#[test]
fn steer_working_rejects_command_prefixes_controls_and_missing_paste_mode() {
    let lab = Lab::new();
    lab.start("claude", "working");
    for text in ["/model", "  !echo hi", "text\u{1b}[A", "text\rsubmit"] {
        lab.reject(&["send", "lab/main", text, "--steer"], 1, "usage");
    }
    assert!(!lab.read().contains("RX:"));
    let lab = Lab::new();
    lab.start("claude", "no_paste");
    lab.reject(
        &["send", "lab/main", "first\nsecond", "--steer"],
        9,
        "unsupported",
    );
    assert!(!lab.read().contains("RX:"));
}
