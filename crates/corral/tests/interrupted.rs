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
    fn hook(&self, event: &str, body: Value) {
        let instance = self.json(&["status", "test/work"])["instance"].clone();
        let mut hook = Command::new(self.0.path().join("corral"))
            .args(["__hook", event])
            .env("HOME", self.0.path())
            .env("CORRAL_HOME", self.0.path().join("pens"))
            .env("CORRAL_EVENTS", self.0.path().join("pens/test/work/events"))
            .env("CORRAL_INSTANCE", instance.as_str().unwrap())
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        writeln!(hook.stdin.take().unwrap(), "{body}").unwrap();
        assert!(hook.wait().unwrap().success());
    }
    fn transcript_turn(&self) {
        self.hook(
            "UserPromptSubmit",
            json!({
                "session_id":"main", "prompt":"second",
                "transcript_path":self.0.path().join("transcript.jsonl")
            }),
        );
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
    fn omp_event(&self, ev: &str, body: Value) {
        self.event(
            ev,
            json!({"v":2,"adapter":"omp","cwd":"/tmp","has_ui":true,"event":body}),
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
fn omp_interrupt_uses_r22_behavior_and_replays_legacy_completion() {
    let lab = Lab::new();
    lab.omp_event("before_agent_start", json!({"prompt":"second"}));
    let id = lab.reminder("15");
    let message = json!({"role":"assistant","content":[],"stopReason":"aborted","errorMessage":"Interrupted by user","errorId":67112960});
    lab.omp_event("message_end", json!({"message":message}));
    lab.omp_event("agent_end", json!({"messages":[message]}));
    assert_eq!(
        lab.json(&["wait", "test/work", "--timeout", "3"])["result"],
        "interrupted"
    );
    let st = lab.json(&["status", "test/work"]);
    assert_eq!(st["state"], "idle");
    assert_eq!(st["interrupted"], true);
    let out = lab.out(&["reply", "test/work"]);
    assert_eq!(out.status.code(), Some(1));
    let error: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(error["error"], "no_reply");
    assert_eq!(error["interrupted"], true);
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(lab.record(&id)["wait_done"], false);

    // A pre-R25 reader has consumed these same events as a normal completion.
    let cursor_path = lab.0.path().join("pens/test/work/cursor");
    let mut old: Value = serde_json::from_slice(&fs::read(&cursor_path).unwrap()).unwrap();
    old["cursor"] = json!(7);
    old["last_event"] = json!("Stop");
    old["interrupted"] = json!(false);
    fs::write(cursor_path, old.to_string()).unwrap();
    assert_eq!(lab.json(&["status", "test/work"])["interrupted"], true);

    lab.omp_event("before_agent_start", json!({"prompt":"continue"}));
    assert_eq!(lab.json(&["status", "test/work"])["interrupted"], false);
    let message = json!({"role":"assistant","content":[{"type":"text","text":"finished now"}],"stopReason":"stop"});
    lab.omp_event("message_end", json!({"message":message}));
    lab.omp_event("agent_end", json!({"messages":[message]}));
    assert_eq!(lab.delivered(&id)["wait_result"]["result"], "idle");
    assert_eq!(
        lab.json(&["wait", "test/work", "--timeout", "3"])["result"],
        "idle"
    );
    assert_eq!(lab.json(&["reply", "test/work"])["text"], "finished now");
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

fn claude_cursor_recovery(legacy: bool) {
    let lab = Lab::new();
    lab.event("Stop", json!({"background_running":0}));
    lab.transcript_turn();
    let cursor_path = lab.0.path().join("pens/test/work/cursor");
    lab.json(&["status", "test/work"]);
    let stale = fs::read(&cursor_path).unwrap();
    lab.transcript("[Request interrupted by user]", "2024-01-01T00:00:01.000Z");
    assert_eq!(lab.json(&["status", "test/work"])["interrupted"], true);
    // Reader A's delayed write overwrites reader B's detected interruption.
    fs::write(&cursor_path, &stale).unwrap();
    lab.event("Notification", json!({"notification_type":"idle_prompt"}));
    if legacy {
        // A v5 reader replays these hooks, producing idle without interruption evidence.
        let mut old: Value = serde_json::from_slice(&stale).unwrap();
        old["cursor"] = json!(5);
        old["state"] = json!("idle");
        old["last_event"] = json!("Notification");
        old["offset"] = json!(
            fs::metadata(lab.0.path().join("pens/test/work/events"))
                .unwrap()
                .len()
        );
        old.as_object_mut().unwrap().remove("transcript_path");
        old.as_object_mut().unwrap().remove("turn_open");
        fs::write(&cursor_path, old.to_string()).unwrap();
    } else {
        // event() queries status first; restore A again so no intermediate detection survives.
        fs::write(&cursor_path, &stale).unwrap();
    }
    let st = lab.json(&["status", "test/work"]);
    assert_eq!(st["state"], "idle");
    assert_eq!(st["interrupted"], true);
    assert_eq!(
        lab.json(&["wait", "test/work", "--timeout", "3"])["result"],
        "interrupted"
    );
    let reply = lab.out(&["reply", "test/work"]);
    assert_eq!(reply.status.code(), Some(1));
    assert_eq!(
        serde_json::from_slice::<Value>(&reply.stdout).unwrap()["interrupted"],
        true
    );
    // A real end remains authoritative even after another notification and a cache rebuild.
    for end in ["Stop", "StopFailure"] {
        lab.event(
            end,
            json!({"last_assistant_message":"finished", "background_running":0}),
        );
        lab.event("Notification", json!({"notification_type":"idle_prompt"}));
        fs::remove_file(&cursor_path).unwrap();
        assert_eq!(lab.json(&["status", "test/work"])["interrupted"], false);
    }
    lab.event(
        "UserPromptSubmit",
        json!({"prompt":"continue", "t":1704067202.0}),
    );
    lab.event("Notification", json!({"notification_type":"idle_prompt"}));
    fs::remove_file(&cursor_path).unwrap();
    assert_eq!(lab.json(&["status", "test/work"])["interrupted"], false);
}

#[test]
fn claude_recovers_interruption_after_stale_reader_write() {
    claude_cursor_recovery(false);
}

#[test]
fn claude_recovers_interruption_after_legacy_cursor_rebuild() {
    claude_cursor_recovery(true);
}

#[test]
fn claude_fifo_and_symlink_do_not_block_status_or_wait() {
    use std::{ffi::CString, os::unix::fs::symlink};
    let lab = Lab::new();
    lab.transcript_turn();
    let transcript = lab.0.path().join("transcript.jsonl");
    let fifo = lab.0.path().join("fifo");
    let name = CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    for linked in [false, true] {
        if linked {
            fs::rename(&transcript, &fifo).unwrap();
            symlink(&fifo, &transcript).unwrap();
        } else {
            fs::rename(&fifo, &transcript).unwrap();
        }
        for args in [
            vec!["status", "test/work"],
            vec!["wait", "test/work", "--timeout", "0.2"],
        ] {
            let mut command = Command::new(lab.0.path().join("corral"));
            command
                .args(&args)
                .env("HOME", lab.0.path())
                .env("CORRAL_HOME", lab.0.path().join("pens"))
                .env("SHELL", "/bin/sh")
                .env_remove("CODEX_SANDBOX")
                .stdout(Stdio::piped());
            let mut child = command.spawn().unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                if child.try_wait().unwrap().is_some() {
                    break;
                }
                if Instant::now() >= deadline {
                    child.kill().unwrap();
                    child.wait().unwrap();
                    panic!("{args:?} blocked on FIFO (symlink={linked})");
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let out = child.wait_with_output().unwrap();
            let value: Value = serde_json::from_slice(&out.stdout).unwrap();
            if args[0] == "status" {
                assert_eq!(value["state"], "working");
                assert_eq!(value["interrupted"], false);
            } else {
                assert_eq!(out.status.code(), Some(4), "{value}");
                assert_eq!(value["error"], "timeout");
            }
        }
    }
}

#[test]
fn claude_sidechain_with_main_session_id_is_not_an_interruption() {
    let lab = Lab::new();
    lab.transcript_turn();
    lab.transcript("[Request interrupted by user]", "2024-01-01T00:00:01.000Z");
    let path = lab.0.path().join("transcript.jsonl");
    let mut record: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    record["isSidechain"] = json!(true);
    record["agentId"] = json!("subagent");
    fs::write(path, format!("{record}\n")).unwrap();
    let st = lab.json(&["status", "test/work"]);
    assert_eq!(st["state"], "working");
    assert_eq!(st["interrupted"], false);
}

fn claude_late_tool_hook(event: &str) {
    let lab = Lab::new();
    lab.transcript_turn();
    lab.transcript("[Request interrupted by user]", "2024-01-01T00:00:01.000Z");
    assert_eq!(lab.json(&["status", "test/work"])["state"], "idle");
    let tool = json!({
        "session_id":"main", "transcript_path":lab.0.path().join("transcript.jsonl"),
        "agent_id":"background-agent", "tool_name":"Bash"
    });
    lab.hook(event, tool.clone());
    for _ in 0..2 {
        let st = lab.json(&["status", "test/work"]);
        assert_eq!(st["state"], "idle", "{event}: {st}");
        assert_eq!(st["interrupted"], true);
    }
    assert_eq!(
        lab.json(&["wait", "test/work", "--timeout", "3"])["result"],
        "interrupted"
    );
    let reply = lab.out(&["reply", "test/work"]);
    assert_eq!(reply.status.code(), Some(1));
    assert_eq!(
        serde_json::from_slice::<Value>(&reply.stdout).unwrap()["interrupted"],
        true
    );
    // The real send reaches cat; a synthetic hook acknowledges the new turn after receipt.
    std::thread::scope(|scope| {
        let send = scope.spawn(|| lab.json(&["send", "test/work", "continue", "--timeout", "5"]));
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if String::from_utf8_lossy(&lab.out(&["read", "test/work"]).stdout).contains("continue")
            {
                break;
            }
            assert!(Instant::now() < deadline, "input did not reach cat");
            std::thread::sleep(Duration::from_millis(50));
        }
        lab.hook(
            "UserPromptSubmit",
            json!({
                "session_id":"main", "prompt":"continue",
                "transcript_path":lab.0.path().join("transcript.jsonl")
            }),
        );
        assert_eq!(send.join().unwrap()["confirmed"], true);
    });
    // The old marker remains on disk while the new prompt hook is already visible.
    let st = lab.json(&["status", "test/work"]);
    assert_eq!(st["state"], "working");
    assert_eq!(st["interrupted"], false);
    lab.hook(
        "Stop",
        json!({"session_id":"main", "last_assistant_message":"new reply"}),
    );
    assert_eq!(
        lab.json(&["wait", "test/work", "--timeout", "3"])["result"],
        "idle"
    );
    assert_eq!(lab.json(&["reply", "test/work"])["text"], "new reply");
    // Even a current marker cannot reopen a normally ended turn after another tool hook.
    for end in ["Stop", "StopFailure"] {
        lab.transcript_turn();
        lab.hook(
            end,
            json!({"session_id":"main", "last_assistant_message":"finished"}),
        );
        lab.hook(event, tool.clone());
        let st = lab.json(&["status", "test/work"]);
        assert_eq!(st["state"], "working");
        assert_eq!(st["interrupted"], false);
    }
}

#[test]
fn claude_late_pre_tool_hook_preserves_interruption() {
    claude_late_tool_hook("PreToolUse");
}

#[test]
fn claude_late_post_tool_hook_preserves_interruption() {
    claude_late_tool_hook("PostToolUse");
}

fn claude_dialog_escape(tool: &str, approved: bool) {
    let lab = Lab::new();
    lab.transcript_turn();
    lab.hook("PreToolUse", json!({"session_id":"main", "tool_name":tool}));
    lab.hook(
        "PermissionRequest",
        json!({"session_id":"main", "tool_name":tool}),
    );
    lab.hook(
        "Notification",
        json!({"session_id":"main", "notification_type":"permission_prompt"}),
    );
    // Neither an open dialog nor a marker from the previous turn is an interruption.
    assert_eq!(lab.json(&["status", "test/work"])["state"], "blocked");
    for timestamp in ["2023-12-31T23:59:59.000Z", "2024-01-01T00:00:00.000Z"] {
        lab.transcript("[Request interrupted by user for tool use]", timestamp);
        let st = lab.json(&["status", "test/work"]);
        assert_eq!(st["state"], "blocked");
        assert_eq!(st["interrupted"], false);
    }
    // R42t: approval and the running command do not produce another corral hook.
    lab.transcript(
        "[Request interrupted by user for tool use]",
        "2024-01-01T00:00:01.000Z",
    );
    if approved {
        let path = lab.0.path().join("transcript.jsonl");
        let marker = fs::read_to_string(&path).unwrap();
        let result = json!({"type":"user", "sessionId":"main",
            "timestamp":"2024-01-01T00:00:00.900Z",
            "message":{"content":[{"type":"tool_result", "content":"ping stopped", "is_error":true}]},
            "toolUseResult":"User rejected tool use"});
        fs::write(path, format!("{result}\n{marker}")).unwrap();
    }
    let st = lab.json(&["status", "test/work"]);
    assert_eq!(st["state"], "idle");
    assert_eq!(st["interrupted"], true);
    std::thread::scope(|scope| {
        let send = scope
            .spawn(|| lab.json(&["send", "test/work", "after-dialog-escape", "--timeout", "5"]));
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if String::from_utf8_lossy(&lab.out(&["read", "test/work"]).stdout)
                .contains("after-dialog-escape")
            {
                break;
            }
            assert!(Instant::now() < deadline, "input did not reach cat");
            std::thread::sleep(Duration::from_millis(50));
        }
        lab.hook(
            "UserPromptSubmit",
            json!({"session_id":"main", "prompt":"after-dialog-escape"}),
        );
        assert_eq!(send.join().unwrap()["confirmed"], true);
    });
}

#[test]
fn claude_permission_dialog_escape_recovers_idle() {
    claude_dialog_escape("Bash", false);
}

#[test]
fn claude_question_dialog_escape_recovers_idle() {
    claude_dialog_escape("AskUserQuestion", false);
}

#[test]
fn claude_plan_dialog_escape_recovers_idle() {
    claude_dialog_escape("ExitPlanMode", false);
}

#[test]
fn claude_approved_tool_escape_recovers_idle() {
    claude_dialog_escape("Bash", true);
}

#[test]
fn claude_question_dialog_exposes_single_choice() {
    let lab = Lab::new();
    lab.transcript_turn();
    // https://code.claude.com/docs/en/hooks#pretooluse-input (AskUserQuestion).
    let body = json!({"session_id":"main", "tool_name":"AskUserQuestion", "tool_input":{
        "questions":[{"header":"颜色", "question":"本次测试选哪种颜色？",
            "options":[{"label":"红色", "description":"暖色测试"},
                       {"label":"蓝色", "description":"冷色测试"}], "multiSelect":false}],
        "answers":{"本次测试选哪种颜色？":"不应记录"}
    }});
    lab.hook("PreToolUse", body.clone());
    assert!(lab.json(&["status", "test/work"])["questions"].is_null());
    lab.hook("PermissionRequest", body);
    lab.hook(
        "Notification",
        json!({"session_id":"main", "notification_type":"permission_prompt"}),
    );
    let st = lab.json(&["status", "test/work"]);
    assert_eq!(st["state"], "blocked");
    assert_eq!(
        st["questions"],
        json!({"items":[{
        "header":"颜色", "question":"本次测试选哪种颜色？", "multi_select":false,
        "options":[{"label":"红色", "description":"暖色测试"},
                   {"label":"蓝色", "description":"冷色测试"}]
    }], "truncated":false})
    );
    let events = fs::read_to_string(lab.0.path().join("pens/test/work/events")).unwrap();
    assert!(!events.contains("不应记录"));
    assert!(!events.contains("tool_input"));
    lab.transcript(
        "[Request interrupted by user for tool use]",
        "2024-01-01T00:00:01.000Z",
    );
    assert!(lab.json(&["status", "test/work"])["questions"].is_null());
}

#[test]
fn claude_questions_truncate_at_a_total_json_limit() {
    let lab = Lab::new();
    let long = "中文\"\n\u{0}".repeat(8192);
    lab.hook(
        "PermissionRequest",
        json!({"session_id":"main", "tool_name":"AskUserQuestion",
        "tool_input":{"questions":[{"header":"长题", "question":long,
            "multiSelect":true, "options":[{"label":"一", "description":"说明"}]}]}}),
    );
    let questions = lab.json(&["status", "test/work"])["questions"].clone();
    assert_eq!(questions["truncated"], true);
    assert!(serde_json::to_vec(&questions).unwrap().len() <= 16 * 1024);
    assert_eq!(questions["items"][0]["header"], "长题");
    let text = questions["items"][0]["question"].as_str().unwrap();
    assert!(!text.is_empty() && long.starts_with(text) && text.len() < long.len());
    for line in fs::read_to_string(lab.0.path().join("pens/test/work/events"))
        .unwrap()
        .lines()
    {
        let event: Value = serde_json::from_str(line).unwrap();
        if !event["questions"].is_null() {
            assert_eq!(event["questions"], questions);
        }
    }
}

fn fruit_question() -> Value {
    json!({"header":"水果", "question":"选择哪些水果？", "multiSelect":true,
        "options":[{"label":"苹果", "description":"脆口"}, {"label":"香蕉", "description":"软口"}]})
}

#[test]
fn claude_questions_preserve_multiple_choice() {
    let lab = Lab::new();
    lab.hook(
        "PermissionRequest",
        json!({"session_id":"main", "tool_name":"AskUserQuestion",
        "tool_input":{"questions":[fruit_question()]}}),
    );
    assert_eq!(
        lab.json(&["status", "test/work"])["questions"],
        json!({"items":[{
        "header":"水果", "question":"选择哪些水果？", "multi_select":true,
        "options":[{"label":"苹果", "description":"脆口"}, {"label":"香蕉", "description":"软口"}]
    }], "truncated":false})
    );
}

#[test]
fn claude_questions_preserve_all_questions_and_clear_after_the_dialog() {
    let lab = Lab::new();
    let second = json!({"header":"颜色", "question":"选哪种颜色？",
        "options":[{"label":"蓝色", "description":"冷色"}, {"label":"红色", "description":"暖色"}]});
    lab.hook(
        "PermissionRequest",
        json!({"session_id":"main", "tool_name":"AskUserQuestion",
        "tool_input":{"questions":[fruit_question(), second]}}),
    );
    assert_eq!(
        lab.json(&["status", "test/work"])["questions"],
        json!({"items":[{
        "header":"水果", "question":"选择哪些水果？", "multi_select":true,
        "options":[{"label":"苹果", "description":"脆口"}, {"label":"香蕉", "description":"软口"}]
    }, {
        "header":"颜色", "question":"选哪种颜色？", "multi_select":false,
        "options":[{"label":"蓝色", "description":"冷色"}, {"label":"红色", "description":"暖色"}]
    }], "truncated":false})
    );
    lab.hook(
        "PostToolUse",
        json!({"session_id":"main", "tool_name":"AskUserQuestion"}),
    );
    assert!(lab.json(&["status", "test/work"])["questions"].is_null());
    lab.hook(
        "Notification",
        json!({"session_id":"main", "notification_type":"permission_prompt"}),
    );
    assert!(lab.json(&["status", "test/work"])["questions"].is_null());
}

#[test]
fn claude_questions_do_not_record_other_tool_parameters() {
    let lab = Lab::new();
    for tool in ["Bash", "ExitPlanMode", "Read"] {
        lab.hook("PermissionRequest", json!({"session_id":"main", "tool_name":tool,
            "tool_input":{"questions":[fruit_question()], "command":"not-recorded", "plan":"not-recorded"}}));
        assert!(lab.json(&["status", "test/work"])["questions"].is_null());
    }
    let events = fs::read_to_string(lab.0.path().join("pens/test/work/events")).unwrap();
    assert!(!events.contains("questions") && !events.contains("not-recorded"));
}

#[test]
#[ignore = "set CORRAL_COMPAT_BIN to the pre-R43 corral binary"]
fn claude_questions_new_and_old_clients_share_events() {
    let lab = Lab::new();
    lab.hook(
        "PermissionRequest",
        json!({"session_id":"main", "tool_name":"AskUserQuestion",
        "tool_input":{"questions":[fruit_question()]}}),
    );
    let expected = lab.json(&["status", "test/work"])["questions"].clone();
    assert!(!expected.is_null());
    for _ in 0..2 {
        let out = Command::new(std::env::var_os("CORRAL_COMPAT_BIN").expect("baseline required"))
            .args(["status", "test/work"])
            .env("HOME", lab.0.path())
            .env("CORRAL_HOME", lab.0.path().join("pens"))
            .env("SHELL", "/bin/sh")
            .env_remove("CODEX_SANDBOX")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
        let old: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(old["state"], "blocked");
        assert_eq!(old["last_tool"], "AskUserQuestion");
        assert_eq!(lab.json(&["status", "test/work"])["questions"], expected);
    }
    // An older hook's next dialog has no questions; do not reuse the previous dialog's text.
    lab.event("PermissionRequest", json!({"tool_name":"AskUserQuestion"}));
    let st = lab.json(&["status", "test/work"]);
    assert_eq!(st["state"], "blocked");
    assert!(st["questions"].is_null());
}
