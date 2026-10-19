//! Interrupted turns must not look like completed delegated work.
mod common;
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    process::{Command, Output, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Lab(tempfile::TempDir);
impl Lab {
    fn new() -> Self {
        let lab = Self(tempfile::tempdir_in("/tmp").unwrap());
        common::copy_exe(env!("CARGO_BIN_EXE_corral"), lab.0.path().join("corral"));
        let agent = lab.0.path().join("claude");
        fs::write(&agent, "#!/bin/sh\nexec /bin/cat\n").unwrap();
        fs::set_permissions(&agent, fs::Permissions::from_mode(0o700)).unwrap();
        lab.json(&[
            "start",
            "test/work",
            "--cwd",
            "/tmp",
            "--",
            agent.to_str().unwrap(),
        ]);
        lab.json(&["start", "test/controller", "--", "/bin/cat"]);
        lab.event("SessionStart", json!({"cwd":"/tmp","has_transcript":true}));
        lab.event("UserPromptSubmit", json!({"prompt":"first"}));
        lab.event("Stop", json!({"last_assistant_message":"previous reply"}));
        lab
    }
    fn out(&self, args: &[&str]) -> Output {
        Command::new(self.0.path().join("corral"))
            .args(args)
            .env("HOME", self.0.path())
            .env("CORRAL_HOME", self.0.path().join("pens"))
            .env("SHELL", "/bin/sh")
            .env_remove("CODEX_SANDBOX")
            .output()
            .unwrap()
    }
    fn json(&self, args: &[&str]) -> Value {
        let out = self.out(args);
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stdout)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
    fn event(&self, ev: &str, extra: Value) {
        let instance = self.json(&["status", "test/work"])["instance"].clone();
        let mut e = json!({"v":1,"inst":instance,"session_id":"main","ev":ev,"t":SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64()});
        e.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        writeln!(
            fs::OpenOptions::new()
                .append(true)
                .open(self.0.path().join("pens/test/work/events"))
                .unwrap(),
            "{e}"
        )
        .unwrap();
    }
    fn interrupt(&self) {
        self.event("UserPromptSubmit", json!({"prompt":"second"}));
        self.event("Interrupt", json!({}));
    }
    fn transcript_turn(&self) {
        let instance = self.json(&["status", "test/work"])["instance"].clone();
        let mut hook = Command::new(self.0.path().join("corral"))
            .args(["__hook", "UserPromptSubmit"])
            .env("CORRAL_EVENTS", self.0.path().join("pens/test/work/events"))
            .env("CORRAL_INSTANCE", instance.as_str().unwrap())
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        writeln!(
            hook.stdin.take().unwrap(),
            "{}",
            json!({
                "session_id":"main", "prompt":"second",
                "transcript_path":self.0.path().join("transcript.jsonl")
            })
        )
        .unwrap();
        assert!(hook.wait().unwrap().success());
        self.event(
            "UserPromptSubmit",
            json!({"prompt":"second", "t":1704067200.0}),
        );
    }
    fn transcript(&self, text: &str, timestamp: &str) {
        fs::write(
            self.0.path().join("transcript.jsonl"),
            format!(
                "{}\n",
                json!({
                    "type":"user", "sessionId":"main", "timestamp":timestamp,
                    "message":{"role":"user", "content":[{"type":"text", "text":text}]}
                })
            ),
        )
        .unwrap();
    }
    fn pi_event(&self, ev: &str, body: Value, idle: bool) {
        self.event(
            ev,
            json!({"v":2,"adapter":"pi","cwd":"/tmp","has_ui":true,"idle":idle,"event":body}),
        );
    }
    fn reminder(&self, timeout: &str) -> String {
        self.json(&[
            "send",
            "test/controller",
            "work-ended",
            "--after",
            "test/work",
            "--timeout",
            timeout,
        ])["request_id"]
            .as_str()
            .unwrap()
            .to_owned()
    }
    fn record(&self, id: &str) -> Value {
        self.json(&["after", "test/controller", "--request-id", id])["records"][0].clone()
    }
    fn delivered(&self, id: &str) -> Value {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let r = self.record(id);
            if r["phase"] == "sent" {
                return r;
            }
            assert!(Instant::now() < deadline, "{r}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

#[test]
fn claude_transcript_interrupt_uses_r22_behavior_and_recovers() {
    let lab = Lab::new();
    lab.event("Stop", json!({"background_running":0}));
    for marker in [
        "[Request interrupted by user]",
        "[Request interrupted by user for tool use]",
    ] {
        lab.transcript_turn();
        let id = lab.reminder("15");
        lab.transcript(marker, "2024-01-01T00:00:01.000Z");
        let st = lab.json(&["status", "test/work"]);
        assert_eq!(st["state"], "idle");
        assert_eq!(st["interrupted"], true);
        // A later idle notification must not turn a detected interruption into completion.
        lab.event(
            "Notification",
            json!({"notification_type":"idle_prompt", "background_running":0}),
        );
        assert_eq!(lab.json(&["status", "test/work"])["interrupted"], true);
        assert_eq!(
            lab.json(&["wait", "test/work", "--timeout", "3"])["result"],
            "interrupted"
        );
        let out = lab.out(&["reply", "test/work"]);
        assert_eq!(out.status.code(), Some(1));
        let error: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(error["error"], "no_reply");
        assert_eq!(error["interrupted"], true);
        std::thread::sleep(Duration::from_millis(1500));
        assert_eq!(lab.record(&id)["wait_done"], false);
        // Acknowledge only after the synthetic agent actually receives this input.
        let text = format!("continue {marker}");
        std::thread::scope(|scope| {
            let send = scope.spawn(|| lab.json(&["send", "test/work", &text, "--timeout", "5"]));
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let output = lab.out(&["read", "test/work"]);
                if String::from_utf8_lossy(&output.stdout).contains(&text) {
                    break;
                }
                assert!(Instant::now() < deadline, "input did not reach cat");
                std::thread::sleep(Duration::from_millis(50));
            }
            lab.event("UserPromptSubmit", json!({"prompt":text}));
            assert_eq!(send.join().unwrap()["confirmed"], true);
        });
        // The new hook can arrive before Claude writes the new user record.
        let st = lab.json(&["status", "test/work"]);
        assert_eq!(st["state"], "working");
        assert_eq!(st["interrupted"], false);
        lab.transcript("continue", "2024-01-01T00:00:02.000Z");
        lab.event("Stop", json!({"last_assistant_message":"new reply"}));
        assert_eq!(lab.json(&["status", "test/work"])["interrupted"], false);
        assert_eq!(lab.json(&["reply", "test/work"])["text"], "new reply");
        assert_eq!(lab.delivered(&id)["wait_result"]["result"], "idle");
    }
}
impl Drop for Lab {
    fn drop(&mut self) {
        for name in ["test/work", "test/controller"] {
            let _ = self.out(&["stop", name, "--timeout", "5"]);
        }
    }
}

#[test]
fn status_records_interruption_until_new_input() {
    let lab = Lab::new();
    lab.interrupt();
    let st = lab.json(&["status", "test/work"]);
    assert_eq!(st["state"], "idle");
    assert_eq!(st["interrupted"], true);
    lab.event("UserPromptSubmit", json!({"prompt":"continue"}));
    assert_eq!(lab.json(&["status", "test/work"])["interrupted"], false);
}

#[test]
fn wait_reports_interrupted_instead_of_idle() {
    let lab = Lab::new();
    lab.interrupt();
    assert_eq!(
        lab.json(&["wait", "test/work", "--timeout", "3"])["result"],
        "interrupted"
    );
}

#[test]
fn reply_does_not_return_the_previous_turn_after_interrupt() {
    let lab = Lab::new();
    lab.interrupt();
    let out = lab.out(&["reply", "test/work"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let error: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(error["error"], "no_reply");
    assert_eq!(error["interrupted"], true);
}

#[test]
fn reminder_waits_through_interrupt_then_delivers_after_normal_turn() {
    let lab = Lab::new();
    lab.event("UserPromptSubmit", json!({"prompt":"second"}));
    let id = lab.reminder("10");
    lab.event("Interrupt", json!({}));
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(lab.record(&id)["wait_done"], false);
    lab.event("UserPromptSubmit", json!({"prompt":"continue"}));
    assert_eq!(lab.json(&["status", "test/work"])["interrupted"], false);
    lab.event("Stop", json!({"last_assistant_message":"finished now"}));
    assert_eq!(lab.delivered(&id)["wait_result"]["result"], "idle");
    assert_eq!(
        lab.json(&["wait", "test/work", "--timeout", "3"])["result"],
        "idle"
    );
    assert_eq!(lab.json(&["reply", "test/work"])["text"], "finished now");
}

#[test]
fn interruption_does_not_disable_reminder_timeout() {
    let lab = Lab::new();
    lab.interrupt();
    let id = lab.reminder("1.5");
    assert_eq!(lab.delivered(&id)["wait_result"]["result"], "timeout");
}

#[test]
fn normal_stop_and_stop_failure_are_not_interruptions() {
    let lab = Lab::new();
    for ev in ["Stop", "StopFailure"] {
        lab.event("UserPromptSubmit", json!({"prompt":"work"}));
        lab.event(ev, json!({"last_assistant_message":"normal reply"}));
        assert_eq!(lab.json(&["status", "test/work"])["interrupted"], false);
        assert_eq!(
            lab.json(&["wait", "test/work", "--timeout", "3"])["result"],
            "idle"
        );
        assert_eq!(lab.json(&["reply", "test/work"])["text"], "normal reply");
        let id = lab.reminder("10");
        assert_eq!(lab.delivered(&id)["wait_result"]["result"], "idle");
    }
}

#[test]
fn pi_ui_close_preserves_interruption_and_reply_protection() {
    let lab = Lab::new();
    lab.pi_event("before_agent_start", json!({"prompt":"second"}), false);
    lab.pi_event("agent_settled", json!({"aborted":true}), true);
    lab.pi_event("ui_prompt_start", json!({}), true);
    lab.pi_event("ui_prompt_end", json!({}), true);
    let st = lab.json(&["status", "test/work"]);
    assert_eq!(st["state"], "idle");
    assert_eq!(st["interrupted"], true);
    assert_eq!(
        lab.json(&["wait", "test/work", "--timeout", "3"])["result"],
        "interrupted"
    );
    let out = lab.out(&["reply", "test/work"]);
    assert_eq!(out.status.code(), Some(1));
    let error: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(error["error"], "no_reply");
    assert_eq!(error["interrupted"], true);
    // Observe the closed dialog, not the legitimate blocked state while it was open.
    let id = lab.reminder("10");
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(lab.record(&id)["wait_done"], false);

    lab.pi_event("before_agent_start", json!({"prompt":"continue"}), false);
    assert_eq!(lab.json(&["status", "test/work"])["interrupted"], false);
    lab.pi_event(
        "message_end",
        json!({"message":{"role":"assistant","content":[{"type":"text","text":"finished now"}]}}),
        false,
    );
    lab.pi_event("agent_settled", json!({"aborted":false}), true);
    assert_eq!(lab.json(&["reply", "test/work"])["text"], "finished now");
    assert_eq!(lab.delivered(&id)["wait_result"]["result"], "idle");
}

#[test]
fn pi_ui_open_after_interruption_still_notifies_blocked() {
    let lab = Lab::new();
    lab.pi_event("before_agent_start", json!({"prompt":"second"}), false);
    lab.pi_event("agent_settled", json!({"aborted":true}), true);
    lab.pi_event("ui_prompt_start", json!({}), true);
    let st = lab.json(&["status", "test/work"]);
    assert_eq!(st["state"], "blocked");
    assert_eq!(st["interrupted"], true);
    assert_eq!(
        lab.json(&["wait", "test/work", "--timeout", "3"])["result"],
        "blocked"
    );
    let id = lab.reminder("1.5");
    assert_eq!(lab.delivered(&id)["wait_result"]["result"], "blocked");
}

#[test]
fn claude_transcript_fallback_and_other_agents_keep_hook_behavior() {
    let lab = Lab::new();
    lab.transcript_turn();
    // No transcript, malformed transcript, and unrecognised markers do not end a turn.
    assert_eq!(lab.json(&["status", "test/work"])["state"], "working");
    fs::write(lab.0.path().join("transcript.jsonl"), "not json\n").unwrap();
    assert_eq!(lab.json(&["status", "test/work"])["state"], "working");
    lab.transcript("[Request cancelled]", "2024-01-01T00:00:01.000Z");
    assert_eq!(lab.json(&["status", "test/work"])["state"], "working");
    lab.transcript("[Request interrupted by user]", "2024-01-01T00:00:01.000Z");
    // R22's reader cached the hook without its path. R24 must replay it.
    let cursor_path = lab.0.path().join("pens/test/work/cursor");
    let mut cursor: Value = serde_json::from_slice(&fs::read(&cursor_path).unwrap()).unwrap();
    cursor["cursor"] = json!(5);
    cursor.as_object_mut().unwrap().remove("transcript_path");
    fs::write(cursor_path, cursor.to_string()).unwrap();
    assert_eq!(lab.json(&["status", "test/work"])["interrupted"], true);
    let meta_path = lab.0.path().join("pens/test/work/meta.json");
    let mut meta: Value = serde_json::from_slice(&fs::read(&meta_path).unwrap()).unwrap();
    for kind in ["codex", "pi", "omp", "amp", "agy"] {
        meta["kind"] = json!(kind);
        fs::write(&meta_path, meta.to_string()).unwrap();
        // A different agent kind starts with its own cursor, never Claude's cached state.
        fs::remove_file(lab.0.path().join("pens/test/work/cursor")).unwrap();
        let st = lab.json(&["status", "test/work"]);
        assert_eq!(st["state"], "working", "{kind}");
        assert_eq!(st["interrupted"], false, "{kind}");
    }
    meta["kind"] = json!("claude");
    fs::write(&meta_path, meta.to_string()).unwrap();
    // An actual Stop stays authoritative even when the tail still contains a marker.
    lab.event("Stop", json!({"last_assistant_message":"normal reply"}));
    let st = lab.json(&["status", "test/work"]);
    assert_eq!(st["state"], "idle");
    assert_eq!(st["interrupted"], false);
    assert_eq!(lab.json(&["reply", "test/work"])["text"], "normal reply");
}
