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
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
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
    fn request(&self, request: Value) -> Value {
        use std::io::BufRead;
        let mut stream = UnixStream::connect(self.root.path().join("pens/lab/main/sock")).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        writeln!(stream, "{request}").unwrap();
        let mut line = String::new();
        std::io::BufReader::new(&stream)
            .read_line(&mut line)
            .unwrap();
        serde_json::from_str(&line).unwrap()
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

// Deliberately malformed cached raw input: exercise a real status-parser panic,
// not a test-only panic hook in the runtime.
fn corrupt_status_cursor(events: &Path) {
    let path = events.with_file_name("cursor");
    let mut cursor: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    cursor["raw"] = json!({"main":{"inputs":null}});
    let temp = events.with_file_name("panic-cursor.tmp");
    fs::write(&temp, serde_json::to_vec(&cursor).unwrap()).unwrap();
    fs::rename(temp, path).unwrap();
    let mut file = fs::OpenOptions::new().append(true).open(events).unwrap();
    writeln!(
        file,
        "{}",
        json!({
            "v":2,"inst":cursor["inst"],"session_id":"main","adapter":"omp","has_ui":true,
            "ev":"input","t":1.0,"event":{"text":"synthetic panic","streamingBehavior":"steer"},
        })
    )
    .unwrap();
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
        if !matches!(mode.as_str(), "idle" | "idle_to_working_on_paste") {
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
            } else if mode == "tool_on_paste" {
                event("PostToolUse", json!({"tool_name":"Bash"}));
            } else if mode == "idle_to_working_on_paste" {
                event("UserPromptSubmit", json!({"prompt":"a different task"}));
            } else if mode == "panic_on_paste" {
                corrupt_status_cursor(Path::new(&std::env::var("CORRAL_EVENTS").unwrap()));
            } else if mode == "block_during_gap" {
                let deadline = Instant::now() + Duration::from_secs(1);
                while Instant::now() < deadline {
                    event("PostToolUse", json!({"tool_name":"Bash"}));
                    std::thread::sleep(Duration::from_millis(20));
                }
                event("PermissionRequest", json!({"tool_name":"AskUserQuestion"}));
            } else if matches!(mode.as_str(), "busy_on_paste" | "briefly_busy_on_paste") {
                println!("BUSY");
                std::io::stdout().flush().unwrap();
                let deadline = Instant::now()
                    + Duration::from_secs(if mode == "busy_on_paste" { 5 } else { 2 });
                while Instant::now() < deadline {
                    event("PostToolUse", json!({"tool_name":"Bash"}));
                    std::thread::sleep(Duration::from_millis(20));
                }
                println!("EVENTS_DONE");
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
        assert_eq!(out.status.code(), Some(3), "{result}");
        assert_eq!(result["error"], "draft_not_submitted");
        assert_eq!(
            result["reason"],
            if mode == "block_on_paste" {
                "not_idle"
            } else {
                "state_changed"
            }
        );
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
fn steer_waits_for_a_quiet_gap_after_a_working_tool_event() {
    let lab = Lab::new();
    lab.start("claude", "tool_on_paste");
    let before = Instant::now();
    let sent = lab.json(&[
        "send",
        "lab/main",
        "correct the task",
        "--steer",
        "--timeout",
        "3",
    ]);
    assert_eq!(sent["confirmed"], true);
    assert!(!sent["request_id"].as_str().unwrap().is_empty());
    assert!(
        before.elapsed() >= Duration::from_millis(550),
        "submitted without a quiet gap"
    );
    assert_eq!(lab.read().matches("RX:0d").count(), 1);
    assert_eq!(
        lab.json(&["status", "lab/main"])["last_input_source"],
        "send"
    );
}

#[test]
fn steer_does_not_retry_an_idle_submission_as_working_input() {
    let lab = Lab::new();
    lab.start("claude", "idle_to_working_on_paste");
    assert_eq!(lab.json(&["status", "lab/main"])["state"], "idle");
    let result = lab.reject(
        &["send", "lab/main", "/model", "--steer"],
        3,
        "draft_not_submitted",
    );
    assert_eq!(result["reason"], "state_changed");
    assert_eq!(result["state"], "working");
    assert!(!lab.read().contains("RX:0d"));
}

#[test]
fn steer_keeps_the_draft_when_working_events_never_leave_a_quiet_gap() {
    let lab = Lab::new();
    lab.start("claude", "busy_on_paste");
    let before = Instant::now();
    let mut sending = lab
        .cmd()
        .args(["send", "lab/main", "draft during tools", "--steer"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    lab.until(|| lab.read().contains("BUSY"));
    // Status remains serviceable during the wait; the submit block is still pending.
    assert_eq!(
        lab.request(json!({"op":"status"}))["recent_sends"][0]["state"],
        "accepted"
    );
    assert!(sending.try_wait().unwrap().is_none());
    let out = sending.wait_with_output().unwrap();
    let result: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(3), "{result}");
    assert_eq!(result["error"], "draft_not_submitted");
    assert!(
        before.elapsed() >= Duration::from_secs(3),
        "did not wait for the gap deadline"
    );
    assert_eq!(result["reason"], "state_changed");
    assert_eq!(result["state"], "working");
    assert_eq!(result["written"], true);
    assert_eq!(result["submitted"], false);
    assert_eq!(result["confirmed"], false);
    lab.until(|| lab.read().contains("EVENTS_DONE"));
    assert!(!lab.read().contains("RX:0d"));
    let status = lab.json(&["status", "lab/main"]);
    assert_eq!(status["recent_sends"][0]["state"], "draft");
    assert_eq!(status["recent_sends"][0]["reason"], "state_changed");
}

#[test]
fn steer_gap_cancels_for_queued_input_and_releases_the_following_blocks() {
    let lab = Lab::new();
    lab.start("claude", "busy_on_paste");
    let sending = lab
        .cmd()
        .args(["send", "lab/main", "draft before queued input", "--steer"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    lab.until(|| lab.read().contains("BUSY"));
    // The normal 300 ms paste delay has elapsed; tool events keep the gap open.
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        lab.request(json!({"op":"status"}))["recent_sends"][0]["state"],
        "accepted"
    );
    let before = Instant::now();
    lab.json(&["keys", "lab/main", "text:z", "text:y"]);
    let elapsed = before.elapsed();
    let out = sending.wait_with_output().unwrap();
    let result: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(3), "{result}");
    assert_eq!(result["error"], "draft_not_submitted");
    assert_eq!(result["reason"], "input_queued");
    assert_eq!(result["written"], true);
    assert_eq!(result["submitted"], false);
    assert_eq!(result["confirmed"], false);
    assert!(elapsed < Duration::from_secs(2), "keys waited {elapsed:?}");
    lab.until(|| lab.read().contains("RX:79"));
    let screen = lab.read();
    assert!(screen.contains("EVENTS_DONE"));
    assert!(screen.contains("RX:7a\nRX:79"), "{screen}");
    assert!(!screen.contains("RX:0d"));
    let status = lab.request(json!({"op":"status"}));
    assert_eq!(status["recent_sends"][0]["state"], "draft");
    assert_eq!(status["recent_sends"][0]["reason"], "input_queued");
}

#[test]
fn steer_gap_cancels_when_the_requesting_client_disconnects() {
    let lab = Lab::new();
    lab.start("claude", "briefly_busy_on_paste");
    let mut sending = lab
        .cmd()
        .args([
            "send",
            "lab/main",
            "draft from disconnected client",
            "--steer",
            "--request-id",
            "gone-test",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    lab.until(|| lab.read().contains("BUSY"));
    std::thread::sleep(Duration::from_millis(500));
    assert!(sending.try_wait().unwrap().is_none());
    sending.kill().unwrap();
    sending.wait().unwrap();
    lab.until(|| lab.request(json!({"op":"status"}))["recent_sends"][0]["state"] != "accepted");
    let status = lab.request(json!({"op":"status"}));
    let record = &status["recent_sends"][0];
    assert_eq!(record["request_id"], "gone-test");
    assert_eq!(record["state"], "draft", "{record}");
    assert_eq!(record["reason"], "client_gone");
    assert_eq!(record["written"], true);
    assert_eq!(record["submitted"], false);
    lab.until(|| lab.read().contains("EVENTS_DONE"));
    // A later byte drains the PTY: no submit key was left pending after the gap.
    lab.json(&["keys", "lab/main", "text:z"]);
    lab.until(|| lab.read().contains("RX:7a"));
    assert!(!lab.read().contains("RX:0d"));
}

#[test]
fn steer_stops_waiting_as_soon_as_the_gap_becomes_blocked() {
    let lab = Lab::new();
    lab.start("claude", "block_during_gap");
    let before = Instant::now();
    let result = lab.reject(
        &["send", "lab/main", "draft before a question", "--steer"],
        3,
        "draft_not_submitted",
    );
    assert_eq!(result["reason"], "not_idle");
    assert_eq!(result["state"], "blocked");
    assert!(
        before.elapsed() < Duration::from_secs(3),
        "waited for the deadline after becoming blocked"
    );
    assert!(!lab.read().contains("RX:0d"));
}

#[test]
fn steer_status_panic_rejects_before_text_and_keeps_the_draft_before_submit() {
    for before_text in [true, false] {
        let lab = Lab::new();
        lab.start(
            "claude",
            if before_text {
                "working"
            } else {
                "panic_on_paste"
            },
        );
        let initial = lab.json(&["status", "lab/main"]);
        if before_text {
            // Hold the body in the input queue until corruption is in place.
            assert_eq!(
                lab.request(json!({"op":"keys","chunks":[{"data":"","delay":10.0}]}))["ok"],
                true
            );
        }
        let sending = lab
            .cmd()
            .args([
                "send",
                "lab/main",
                "guard the parser",
                "--steer",
                "--request-id",
                "panic-test",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        if before_text {
            lab.until(|| {
                lab.request(json!({"op":"status"}))["recent_sends"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["request_id"] == "panic-test")
            });
            corrupt_status_cursor(&lab.root.path().join("pens/lab/main/events"));
        }
        let out = sending.wait_with_output().unwrap();
        let result: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(
            out.status.code(),
            Some(if before_text { 1 } else { 3 }),
            "{result}"
        );
        assert_eq!(
            result["error"],
            if before_text {
                "state_check_failed"
            } else {
                "draft_not_submitted"
            }
        );
        if !before_text {
            assert_eq!(result["reason"], "state_check_failed");
        }
        assert_eq!(result["written"], !before_text);
        assert_eq!(result["submitted"], false);
        assert_eq!(result["confirmed"], false);
        let status = lab.request(json!({"op":"status"}));
        assert_eq!(status["instance"], initial["instance"]);
        assert_eq!(status["agent_pid"], initial["agent_pid"]);
        assert_eq!(
            status["recent_sends"][0]["state"],
            if before_text { "rejected" } else { "draft" }
        );
        assert!(!lab.read().contains("RX:0d"));
        if before_text {
            assert!(!lab.read().contains("RX:"));
        }
        // The same pen and synthetic agent still accept unrelated protocol operations.
        lab.json(&["keys", "lab/main", "text:z"]);
        lab.until(|| lab.read().contains("RX:7a"));
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
                    // macOS inherits the listener's nonblocking mode on accepted sockets.
                    socket.set_nonblocking(false).unwrap();
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
