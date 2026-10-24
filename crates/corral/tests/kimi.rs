//! Kimi's public lifecycle with an isolated HOME and synthetic PTY programs.
mod common;
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Output},
    time::{Duration, Instant},
};

struct Lab {
    root: tempfile::TempDir,
    strays: Vec<i32>,
    epoch: f64,
}
impl Lab {
    fn new(script: &str, prompt: Option<&str>) -> Self {
        let mut lab = Self {
            root: tempfile::tempdir_in("/tmp").unwrap(),
            strays: vec![],
            epoch: 0.,
        };
        common::copy_exe(env!("CARGO_BIN_EXE_corral"), lab.path("corral"));
        common::copy_exe(std::env::current_exe().unwrap(), lab.path("child"));
        fs::write(lab.path("kimi"), format!("#!/bin/sh\n{script}\n")).unwrap();
        fs::set_permissions(lab.path("kimi"), fs::Permissions::from_mode(0o700)).unwrap();
        let program = lab.path("kimi");
        let mut args = vec!["start", "test/kimi", "--cwd", "/tmp"];
        if let Some(prompt) = prompt {
            args.extend(["--prompt", prompt]);
        }
        args.extend(["--", program.to_str().unwrap()]);
        lab.json(&args);
        lab.epoch = lab.status()["started"].as_f64().unwrap().ceil();
        lab
    }
    // Keep the synthetic timeline relative to the actual process lifetime.
    fn ms(&self, time: u64) -> u64 {
        (self.epoch * 1000.) as u64 + time - 1_000_000
    }
    fn at(&self, time: f64) -> f64 {
        self.epoch + (time - 1000.)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.root.path().join(name)
    }
    fn out(&self, args: &[&str]) -> Output {
        self.out_with(&self.path("corral"), args)
    }
    fn out_with(&self, binary: &std::path::Path, args: &[&str]) -> Output {
        Command::new(binary)
            .args(args)
            .env("HOME", self.root.path())
            .env("CORRAL_HOME", self.path("pens"))
            .env("KIMI_CODE_HOME", self.path("kimi-home"))
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
    fn status(&self) -> Value {
        self.json(&["status", "test/kimi"])
    }
    fn event(&self, ev: &str, extra: Value) {
        let mut e = json!({"v":1,"inst":self.status()["instance"],"session_id":"session-test","ev":ev,
            "cwd":"/tmp","has_transcript":true,"t":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64()});
        e.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        writeln!(
            fs::OpenOptions::new()
                .append(true)
                .open(self.path("pens/test/kimi/events"))
                .unwrap(),
            "{e}"
        )
        .unwrap();
    }
}
impl Drop for Lab {
    fn drop(&mut self) {
        let _ = self.out(&["stop", "test/kimi", "--timeout", "6"]);
        for &pid in &self.strays {
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
        }
    }
}
fn eventually(mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !check() {
        assert!(Instant::now() < deadline, "condition not reached");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn append_wire(path: &std::path::Path, event: Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    writeln!(
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap(),
        "{event}"
    )
    .unwrap();
}

#[test]
fn review_child_dialogs_count_without_changing_main_state_or_command() {
    let lab = Lab::new("exec /bin/cat", None);
    let main = lab.path("s/agents/main/wire.jsonl");
    let child = lab.path("s/agents/agent-0/wire.jsonl");
    lab.event(
        "SessionStart",
        json!({"transcript_path":main,"source":"startup"}),
    );
    lab.event(
        "UserPromptSubmit",
        json!({"prompt":"go","t":lab.at(1000.0)}),
    );
    append_wire(
        &main,
        json!({"type":"turn.prompt","agentId":"main","turnId":1,"time":lab.ms(1000001)}),
    );
    append_wire(
        &child,
        json!({"type":"interaction.request","agentId":"agent-0","id":"child","kind":"approval","time":lab.ms(1000002),
        "request":{"toolName":"Bash","display":{"kind":"command","command":"rm -rf build"}}}),
    );
    append_wire(
        &main,
        json!({"type":"interaction.request","agentId":"main","id":"main","kind":"approval","time":lab.ms(1000003),
        "request":{"turnId":1,"toolName":"Bash","display":{"kind":"command","command":"echo main"}}}),
    );
    let st = lab.status();
    assert_eq!(st["interaction"]["count"], 2);
    assert_eq!(st["command"]["text"], "echo main");
    append_wire(
        &main,
        json!({"type":"interaction.resolved","agentId":"main","id":"main","time":lab.ms(1000004),"response":{"decision":"approved"}}),
    );
    let st = lab.status();
    assert_eq!(st["state"], "working");
    assert_eq!(
        st["interaction"],
        json!({"kind":null,"count":1,"running":true})
    );
    append_wire(
        &child,
        json!({"type":"interaction.resolved","agentId":"agent-0","id":"child","time":lab.ms(1000005),"response":{"decision":"rejected"}}),
    );
    assert_eq!(lab.status()["interaction"]["count"], 0);
    assert_eq!(lab.status()["resolved"]["decision"], "approved");
    append_wire(
        &child,
        json!({"type":"interaction.request","agentId":"agent-0","id":"question","kind":"question","time":lab.ms(1000006),"request":{}}),
    );
    assert_eq!(lab.status()["interaction"]["count"], 1);
}

#[test]
fn review2_bad_old_child_line_nulls_interaction_forever() {
    let lab = Lab::new("exec /bin/cat", None);
    let main = lab.path("s/agents/main/wire.jsonl");
    let child = lab.path("s/agents/agent-0/wire.jsonl");
    fs::create_dir_all(child.parent().unwrap()).unwrap();
    // A previous process was killed while writing tool output mentioning an interaction.
    fs::write(
        &child,
        format!(
            "{{\"type\":\"context.append_loop_event\",\"time\":{},\"event\":{{\"type\":\"tool.result\",\"output\":\"interaction.request",
            lab.ms(900002)
        ),
    )
    .unwrap();
    lab.event(
        "SessionStart",
        json!({"transcript_path":main,"source":"resume"}),
    );
    lab.event(
        "UserPromptSubmit",
        json!({"prompt":"go","t":lab.at(1000.0)}),
    );
    append_wire(
        &main,
        json!({"type":"turn.prompt","agentId":"main","turnId":2,"time":lab.ms(1000001)}),
    );
    append_wire(
        &main,
        json!({"type":"interaction.request","agentId":"main","id":"main","kind":"approval","time":lab.ms(1000002),
        "request":{"turnId":2,"toolName":"Bash","display":{"kind":"command","command":"echo ok"}}}),
    );
    let st = lab.status();
    assert_eq!(st["state"], "blocked");
    assert_eq!(st["interaction"]["count"], 1, "unfinished old line");
    assert_eq!(st["command"]["text"], "echo ok");

    let mut file = fs::OpenOptions::new().append(true).open(&child).unwrap();
    writeln!(file).unwrap();
    assert_eq!(lab.status()["interaction"]["count"], 1, "complete bad line");
    let request = json!({"type":"interaction.request","agentId":"agent-0","id":"child","kind":"approval","time":lab.ms(1000003)}).to_string();
    let (first, last) = request.as_bytes().split_at(request.len() / 2);
    file.write_all(first).unwrap();
    assert_eq!(lab.status()["interaction"]["count"], 1, "partial new line");
    file.write_all(last).unwrap();
    assert_eq!(lab.status()["interaction"]["count"], 1, "still no newline");
    writeln!(file).unwrap();
    assert_eq!(lab.status()["interaction"]["count"], 2, "now durable");
    append_wire(
        &child,
        json!({"type":"interaction.resolved","agentId":"agent-0","id":"child","time":lab.ms(1000004)}),
    );
    assert_eq!(lab.status()["interaction"]["count"], 1);
    append_wire(
        &main,
        json!({"type":"interaction.resolved","agentId":"main","id":"main","time":lab.ms(1000005),"response":{"decision":"approved"}}),
    );
    assert_eq!(lab.status()["interaction"]["count"], 0);
}

#[test]
fn review_resuming_a_session_ignores_previous_process_dialogs() {
    for kind in ["foreground", "background", "approval"] {
        let lab = Lab::new("exec /bin/cat", None);
        let main = lab.path("s/agents/main/wire.jsonl");
        let child = lab.path("s/agents/agent-0/wire.jsonl");
        append_wire(
            &main,
            json!({"type":"turn.prompt","agentId":"main","turnId":1,"time":lab.ms(900001)}),
        );
        if kind == "background" {
            append_wire(
                &main,
                json!({"type":"context.append_loop_event","agentId":"main","time":lab.ms(900002),
                "event":{"type":"tool.call","name":"AskUserQuestion","toolCallId":"bg","args":{"background":true}}}),
            );
        }
        let old = json!({"type":"interaction.request","agentId":"main","id":"old","time":lab.ms(900003),
            "kind":if kind == "approval" {"approval"} else {"question"},
            "request":{"turnId":1,"toolCallId":"bg","toolName":"Bash","display":{"kind":"command","command":"echo old"},
                "questions":[{"question":"Old?","options":[{"label":"A"},{"label":"B"}]}]}});
        append_wire(&main, old.clone());
        let mut old_child = old.clone();
        old_child["agentId"] = json!("agent-0");
        append_wire(&child, old_child);
        lab.event(
            "SessionStart",
            json!({"transcript_path":main,"source":"resume"}),
        );
        lab.event(
            "UserPromptSubmit",
            json!({"prompt":"continue","t":lab.at(1000.)}),
        );
        append_wire(
            &main,
            json!({"type":"turn.prompt","agentId":"main","turnId":2,"time":lab.ms(1000001)}),
        );
        append_wire(
            &main,
            json!({"type":"turn.ended","agentId":"main","turnId":2,"time":lab.ms(1000002),"reason":"completed"}),
        );
        let st = lab.status();
        assert_eq!(st["state"], "idle", "{kind}");
        assert_eq!(st["interaction"]["count"], 0);
        assert!(st["questions"].is_null() && st["command"].is_null());
        let sent: Value =
            serde_json::from_slice(&lab.out(&["send", "test/kimi", "--", "hello"]).stdout).unwrap();
        assert_ne!(sent["error"], "not_idle", "{sent}");
        // A new request in this process, even reusing the old id, is still actionable.
        let mut current = old;
        current["time"] = json!(lab.ms(1000003));
        current["request"]["turnId"] = json!(2);
        append_wire(&main, current);
        assert_eq!(lab.status()["state"], "blocked");
        assert_eq!(lab.status()["interaction"]["count"], 1);
    }
}

#[test]
fn review_same_kind_dialogs_show_the_oldest_then_advance() {
    for kind in ["approval", "question"] {
        let lab = Lab::new("exec /bin/cat", None);
        let wire = lab.path("wire.jsonl");
        lab.event(
            "SessionStart",
            json!({"transcript_path":wire,"source":"startup"}),
        );
        lab.event("UserPromptSubmit", json!({"prompt":"go","t":lab.at(1000.)}));
        append_wire(
            &wire,
            json!({"type":"turn.prompt","agentId":"main","turnId":1,"time":lab.ms(1000001)}),
        );
        for (id, text, t) in [
            ("first", "echo first", 1000002),
            ("second", "echo second", 1000003),
        ] {
            append_wire(
                &wire,
                json!({"type":"interaction.request","agentId":"main","id":id,"kind":kind,"time":lab.ms(t),
                "request":{"turnId":1,"toolName":"Bash","display":{"kind":"command","command":text},
                    "questions":[{"question":text,"options":[{"label":"A"},{"label":"B"}]}]}}),
            );
        }
        let st = lab.status();
        assert_eq!(st["state_started"], lab.at(1000.002), "{kind}");
        assert_eq!(st["interaction"]["count"], 2);
        assert_eq!(
            if kind == "approval" {
                &st["command"]["text"]
            } else {
                &st["questions"]["items"][0]["question"]
            },
            "echo first"
        );
        append_wire(
            &wire,
            json!({"type":"interaction.resolved","agentId":"main","id":"first","time":lab.ms(1000004),"response":null}),
        );
        let next = lab.status();
        assert_eq!(next["state_started"], lab.at(1000.003));
        assert_eq!(next["interaction"]["count"], 1);
        assert_ne!(next["state_started"], st["state_started"]);
    }
}

#[test]
fn review_resolved_survives_a_new_dialog_before_the_next_poll() {
    for kind in ["approval", "question"] {
        let lab = Lab::new("exec /bin/cat", None);
        let wire = lab.path("wire.jsonl");
        lab.event(
            "SessionStart",
            json!({"transcript_path":wire,"source":"startup"}),
        );
        lab.event("UserPromptSubmit", json!({"prompt":"go","t":lab.at(1000.)}));
        append_wire(
            &wire,
            json!({"type":"turn.prompt","agentId":"main","turnId":1,"time":lab.ms(1000001)}),
        );
        append_wire(
            &wire,
            json!({"type":"interaction.request","agentId":"main","id":"first","kind":kind,"time":lab.ms(1000002),"request":{"turnId":1}}),
        );
        let initial = lab.status();
        let response = if kind == "approval" {
            json!({"decision":"approved"})
        } else {
            json!({"answers":{"Q":"A"}})
        };
        append_wire(
            &wire,
            json!({"type":"interaction.resolved","agentId":"main","id":"first","time":lab.ms(1000003),"response":response}),
        );
        append_wire(
            &wire,
            json!({"type":"turn.ended","agentId":"main","turnId":1,"time":lab.ms(1000004),"reason":"completed"}),
        );
        append_wire(
            &wire,
            json!({"type":"turn.prompt","agentId":"main","turnId":2,"time":lab.ms(1000005),"origin":{"kind":"task"}}),
        );
        append_wire(
            &wire,
            json!({"type":"interaction.request","agentId":"main","id":"next","kind":"approval","time":lab.ms(1000006),"request":{"turnId":2,"toolName":"Bash"}}),
        );
        let st = lab.status();
        assert_eq!(st["resolved"]["state_started"], initial["state_started"]);
        assert_eq!(st["resolved"]["kind"], kind);
        assert_eq!(
            st["resolved"][if kind == "approval" {
                "decision"
            } else {
                "answers"
            }],
            response[if kind == "approval" {
                "decision"
            } else {
                "answers"
            }]
        );
        assert_eq!(st["state_started"], lab.at(1000.006));
        // Cursor replay must recover the same result, not only the warm cache.
        fs::remove_file(lab.path("pens/test/kimi/cursor")).unwrap();
        assert_eq!(lab.status()["resolved"], st["resolved"]);
    }
}

#[test]
fn background_question_survives_tools_turns_and_cursor_replay() {
    let lab = Lab::new("exec /bin/cat", None);
    let wire = lab.path("wire.jsonl");
    lab.event(
        "SessionStart",
        json!({"transcript_path":wire,"source":"startup"}),
    );
    lab.event(
        "UserPromptSubmit",
        json!({"prompt":"ask","t":lab.at(1000.0)}),
    );
    let append = |v: Value| {
        writeln!(
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&wire)
                .unwrap(),
            "{v}"
        )
        .unwrap();
    };
    append(json!({"type":"turn.prompt","agentId":"main","turnId":1,"time":lab.ms(1000001)}));
    append(
        json!({"type":"context.append_loop_event","agentId":"main","time":lab.ms(1000002),
        "event":{"type":"tool.call","name":"AskUserQuestion","turnId":"1","toolCallId":"call-q","args":{"background":true}}}),
    );
    append(
        json!({"type":"interaction.request","agentId":"main","id":"q","kind":"question","time":lab.ms(1000003),
        "request":{"turnId":1,"toolCallId":"call-q","questions":[{"header":"水果","question":"吃什么？","options":[{"label":"苹果"},{"label":"梨"}]}]}}),
    );
    append(
        json!({"type":"context.append_loop_event","agentId":"main","time":lab.ms(1000004),
        "event":{"type":"tool.result","toolCallId":"call-q","result":{"output":"task_id: question-test\nstatus: running"}}}),
    );
    let st = lab.status();
    assert_eq!(st["state"], "blocked");
    assert_eq!(st["questions"]["background"], true);
    assert_eq!(
        st["interaction"],
        json!({"kind":"question","count":1,"running":true})
    );
    assert_eq!(st["state_started"], lab.at(1000.003));
    append(
        json!({"type":"turn.ended","agentId":"main","turnId":1,"time":lab.ms(1000005),"reason":"completed"}),
    );
    assert_eq!(lab.status()["interaction"]["running"], false);
    lab.event(
        "UserPromptSubmit",
        json!({"prompt":"next","t":lab.at(1001.0)}),
    );
    append(json!({"type":"turn.prompt","agentId":"main","turnId":2,"time":lab.ms(1001001)}));
    let st = lab.status();
    assert_eq!(st["questions"]["items"][0]["question"], "吃什么？");
    assert_eq!(st["state_started"], lab.at(1000.003));
    assert_eq!(st["interaction"]["running"], true);
    // Earlier readers can consume the wire and drop fields; v3 must replay both.
    let cursor = lab.path("pens/test/kimi/cursor");
    for version in [1, 2] {
        let mut old: Value = serde_json::from_slice(&fs::read(&cursor).unwrap()).unwrap();
        old["kimi_wire"]["version"] = json!(version);
        old["kimi_wire"]
            .as_object_mut()
            .unwrap()
            .remove("interactions");
        fs::write(&cursor, serde_json::to_vec(&old).unwrap()).unwrap();
        assert_eq!(lab.status()["questions"], st["questions"]);
    }
    append(
        json!({"type":"interaction.resolved","agentId":"main","id":"q","time":lab.ms(1001002),
        "response":{"answers":{"吃什么？":"梨"},"method":"enter"}}),
    );
    let st = lab.status();
    assert_eq!(st["state"], "working");
    assert!(st["questions"].is_null());
    assert_eq!(
        st["resolved"],
        json!({"kind":"question","state_started":lab.at(1000.003),"at":lab.at(1001.002),
        "answers":{"吃什么？":"梨"},"truncated":false})
    );
}

#[test]
fn approval_commands_actions_results_and_modal_order_are_structured() {
    let lab = Lab::new("exec /bin/cat", None);
    let wire = lab.path("wire.jsonl");
    lab.event(
        "SessionStart",
        json!({"transcript_path":wire,"source":"startup"}),
    );
    lab.event(
        "UserPromptSubmit",
        json!({"prompt":"run","t":lab.at(1000.0)}),
    );
    let append = |v: Value| {
        writeln!(
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&wire)
                .unwrap(),
            "{v}"
        )
        .unwrap();
    };
    append(json!({"type":"turn.prompt","agentId":"main","turnId":1,"time":lab.ms(1000001)}));
    let command = "printf  '原样\\n' > out.txt";
    let approval = json!({"type":"interaction.request","agentId":"main","id":"a","kind":"approval","time":lab.ms(1000002),
        "request":{"turnId":1,"toolName":"Bash","action":"Running: shortened…","display":{"kind":"command","command":command}}});
    append(approval.clone());
    let st = lab.status();
    assert_eq!(st["command"], json!({"text":command,"truncated":false}));
    assert_eq!(st["action"]["text"], "Running: shortened…");
    assert_eq!(
        lab.json(&["wait", "test/kimi", "--timeout", "1"])["command"],
        st["command"]
    );
    append(
        json!({"type":"interaction.request","agentId":"main","id":"q","kind":"question","time":lab.ms(1000003),
        "request":{"turnId":1,"questions":[{"header":"Q","question":"First?","options":[{"label":"A"},{"label":"B"}]}]}}),
    );
    let st = lab.status();
    assert_eq!(st["interaction"]["count"], 2);
    assert_eq!(
        st["last_tool"], "Bash",
        "a question queues behind the active approval"
    );
    append(
        json!({"type":"interaction.resolved","agentId":"main","id":"a","time":lab.ms(1000004),"response":{"decision":"approved"}}),
    );
    let st = lab.status();
    assert_eq!(st["last_tool"], "AskUserQuestion");
    assert!(st["command"].is_null());
    assert_eq!(
        st["resolved"],
        json!({"kind":"approval","state_started":lab.at(1000.002),"at":lab.at(1000.004),"decision":"approved"})
    );
    append(approval.clone());
    assert_eq!(
        lab.status()["last_tool"],
        "AskUserQuestion",
        "approval now queues behind the question"
    );
    append(
        json!({"type":"interaction.resolved","agentId":"main","id":"q","time":lab.ms(1000005),"response":null}),
    );
    assert_eq!(lab.status()["resolved"]["answers"], Value::Null);
    append(
        json!({"type":"interaction.resolved","agentId":"main","id":"a","time":lab.ms(1000006),"response":{"decision":"rejected"}}),
    );
    assert_eq!(lab.status()["resolved"]["decision"], "rejected");
    let mut non_bash = approval.clone();
    non_bash["request"]["toolName"] = json!("Write");
    non_bash["request"]["action"] = json!("Writing outside.txt");
    non_bash["request"]["display"]["kind"] = json!("file_io");
    append(non_bash);
    assert!(lab.status()["command"].is_null());
    assert_eq!(lab.status()["action"]["text"], "Writing outside.txt");
    let mut long = approval;
    long["request"]["display"]["command"] = json!("汉\"\\\n".repeat(5000));
    append(long);
    let cmd = lab.status()["command"].clone();
    assert_eq!(cmd["truncated"], true);
    assert!(cmd.to_string().len() <= 16 * 1024);
    assert!(
        "汉\"\\\n"
            .repeat(5000)
            .starts_with(cmd["text"].as_str().unwrap())
    );
}

#[test]
fn undo_picker_blocks_send_and_a_background_answer_does_not_invent_a_turn() {
    let lab = Lab::new(
        "printf 'Select messages to undo  ↑↓ navigate · Enter select · Esc cancel\\n'\nexec /bin/cat",
        None,
    );
    lab.event("SessionStart", json!({"source":"startup"}));
    eventually(|| {
        String::from_utf8_lossy(&lab.out(&["read", "test/kimi"]).stdout).contains("Select messages")
    });
    assert_eq!(lab.status()["state"], "blocked");
    let result: Value =
        serde_json::from_slice(&lab.out(&["send", "test/kimi", "--", "hello"]).stdout).unwrap();
    assert_eq!(result["error"], "not_idle");
    let lab = Lab::new("exec /bin/cat", None);
    let wire = lab.path("wire.jsonl");
    lab.event(
        "SessionStart",
        json!({"transcript_path":wire,"source":"startup"}),
    );
    lab.event(
        "UserPromptSubmit",
        json!({"prompt":"ask","t":lab.at(1000.0)}),
    );
    let events = [
        json!({"type":"turn.prompt","agentId":"main","turnId":1,"time":lab.ms(1000001)}),
        json!({"type":"interaction.request","agentId":"main","id":"q","kind":"question","time":lab.ms(1000002),"request":{"turnId":1}}),
        json!({"type":"turn.ended","agentId":"main","turnId":1,"time":lab.ms(1000003),"reason":"completed"}),
        json!({"type":"interaction.resolved","agentId":"main","id":"q","time":lab.ms(1000004),"response":{"answers":{"Q":"答\"\\\n".repeat(6000)}}}),
    ];
    fs::write(
        wire,
        events.iter().map(|v| format!("{v}\n")).collect::<String>(),
    )
    .unwrap();
    let st = lab.status();
    assert_eq!(st["state"], "idle");
    assert_eq!(st["resolved"]["truncated"], true);
    assert!(st["resolved"].to_string().len() <= 16 * 1024);
}

#[test]
#[ignore = "set CORRAL_COMPAT_BIN to the pre-R48g2 corral"]
fn kimi_new_and_old_readers_alternate_without_losing_background_interactions() {
    let old = PathBuf::from(std::env::var_os("CORRAL_COMPAT_BIN").expect("old corral path"));
    let lab = Lab::new("exec /bin/cat", None);
    let wire = lab.path("wire.jsonl");
    lab.event(
        "SessionStart",
        json!({"transcript_path":wire,"source":"startup"}),
    );
    lab.event(
        "UserPromptSubmit",
        json!({"prompt":"ask","t":lab.at(1000.0)}),
    );
    let append = |v: Value| {
        writeln!(
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&wire)
                .unwrap(),
            "{v}"
        )
        .unwrap();
    };
    append(json!({"type":"turn.prompt","agentId":"main","turnId":1,"time":lab.ms(1000001)}));
    append(
        json!({"type":"context.append_loop_event","agentId":"main","time":lab.ms(1000002),"event":{
        "type":"tool.call","turnId":"1","name":"AskUserQuestion","toolCallId":"bg","args":{"background":true}}}),
    );
    append(
        json!({"type":"interaction.request","agentId":"main","id":"q","kind":"question","time":lab.ms(1000003),"request":{
        "turnId":1,"toolCallId":"bg","questions":[{"question":"Q","header":"Q","options":[{"label":"A"},{"label":"B"}]}]}}),
    );
    append(
        json!({"type":"turn.ended","agentId":"main","turnId":1,"time":lab.ms(1000004),"reason":"completed"}),
    );
    for _ in 0..3 {
        let result = lab.out_with(&old, &["status", "test/kimi"]);
        assert!(result.status.success());
        assert_eq!(
            serde_json::from_slice::<Value>(&result.stdout).unwrap()["state"],
            "idle"
        );
        let new = lab.status();
        assert_eq!(new["questions"]["background"], true);
        assert_eq!(new["interaction"]["running"], false);
        assert_eq!(new["state_started"], lab.at(1000.003));
    }
    append(
        json!({"type":"interaction.resolved","agentId":"main","id":"q","time":lab.ms(1000005),"response":null}),
    );
    assert!(
        lab.out_with(&old, &["status", "test/kimi"])
            .status
            .success()
    );
    assert_eq!(lab.status()["resolved"]["answers"], Value::Null);
    assert!(lab.status()["questions"].is_null());
}

#[test]
fn a_sessionless_editor_accepts_the_first_send_without_force() {
    let lab = Lab::new(
        r#"printf 'No session yet — one will be created on your first message.\n> \nModel   /tmp\ncontext: 0%% (0/1M)\n'
IFS= read -r input
printf '%s\n' '{"session_id":"session-test","cwd":"/tmp","source":"startup"}' | "$CORRAL_HELPER" __hook kimi:SessionStart
printf '%s\n' '{"session_id":"session-test","cwd":"/tmp","prompt":[{"type":"text","text":"-first"}]}' | "$CORRAL_HELPER" __hook kimi:UserPromptSubmit
exec /bin/cat"#,
        None,
    );
    eventually(|| {
        String::from_utf8_lossy(&lab.out(&["read", "test/kimi"]).stdout).contains("No session yet")
    });
    assert_eq!(lab.status()["state"], "idle");
    assert_eq!(lab.json(&["send", "test/kimi", "--", "-first"])["ok"], true);
}

#[test]
fn only_the_main_wire_finishes_a_turn_and_supplies_its_reply() {
    let lab = Lab::new("exec /bin/cat", None);
    let wire = lab.path("wire.jsonl");
    lab.event(
        "SessionStart",
        json!({"transcript_path":wire,"source":"startup"}),
    );
    lab.event(
        "UserPromptSubmit",
        json!({"prompt":"first","t":lab.at(1000.0)}),
    );
    let noise = json!({"type":"model.context","data":"上下文🦀".repeat(4096)});
    let append = |value: Value| {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&wire)
            .unwrap();
        writeln!(file, "{noise}\n{value}").unwrap();
        // Matching keywords must not make malformed UTF-8 into a valid wire event.
        let mut malformed = format!("{{\"type\":\"turn.ended\",\"agentId\":\"main\",\"turnId\":1,\"time\":{},\"reason\":\"cancelled\",\"bad\":\"", lab.ms(1000010)).into_bytes();
        malformed.extend_from_slice(b"\xff\"}\n");
        file.write_all(&malformed).unwrap();
    };
    append(
        json!({"type":"turn.prompt","agentId":"main","turnId":0,"time":1,"input":[{"type":"text","text":"old"}]}),
    );
    append(json!({"type":"turn.ended","agentId":"main","turnId":0,"time":2,"reason":"completed"}));
    append(
        json!({"type":"turn.prompt","agentId":"main","turnId":1,"time":lab.ms(1000001),"input":[{"type":"text","text":"first"}]}),
    );
    append(
        json!({"type":"context.append_loop_event","agentId":"main","time":lab.ms(1000001),"event":{"type":"content.part","turnId":"1","step":0,"part":{"type":"text","text":"I'll check the file."}}}),
    );
    append(
        json!({"type":"context.append_loop_event","agentId":"main","time":lab.ms(1000002),"event":{"type":"tool.call","name":"Bash","turnId":"1"}}),
    );
    let st = lab.status();
    assert_eq!(
        (&st["state"], &st["last_tool"]),
        (&json!("working"), &json!("Bash"))
    );
    append(
        json!({"type":"interaction.request","agentId":"main","id":"approval","kind":"approval","request":{"turnId":1,"toolName":"Bash"},"time":lab.ms(1000003)}),
    );
    assert_eq!(lab.status()["state"], "blocked");
    append(
        json!({"type":"interaction.resolved","agentId":"main","id":"approval","time":lab.ms(1000004)}),
    );
    assert_eq!(lab.status()["state"], "working");
    append(
        json!({"type":"context.append_loop_event","agentId":"main","time":lab.ms(1000004),"event":{"type":"tool.result","turnId":"1"}}),
    );
    assert_eq!(lab.status()["state"], "working");
    append(
        json!({"type":"context.append_loop_event","agentId":"main","time":lab.ms(1000005),"event":{"type":"content.part","turnId":"1","part":{"type":"think","text":"hidden"}}}),
    );
    append(
        json!({"type":"context.append_loop_event","agentId":"main","time":lab.ms(1000006),"event":{"type":"content.part","turnId":"1","step":1,"part":{"type":"text","text":"main "}}}),
    );
    // A child's event and the indistinguishable Stop hook cannot close the main turn.
    append(
        json!({"type":"turn.ended","agentId":"agent-0","turnId":0,"reason":"completed","time":lab.ms(1000007)}),
    );
    lab.event("Kimi:Stop", json!({}));
    assert_eq!(lab.status()["state"], "working");
    assert!(!lab.out(&["reply", "test/kimi"]).status.success());
    append(
        json!({"type":"context.append_loop_event","agentId":"main","time":lab.ms(1000008),"event":{"type":"content.part","turnId":"1","step":1,"part":{"type":"text","text":"reply"}}}),
    );
    let end = json!({"type":"turn.ended","agentId":"main","turnId":1,"reason":"completed","time":lab.ms(1000009)});
    write!(
        fs::OpenOptions::new().append(true).open(&wire).unwrap(),
        "{end}"
    )
    .unwrap();
    assert_eq!(
        lab.status()["state"],
        "working",
        "partial line is not committed"
    );
    writeln!(fs::OpenOptions::new().append(true).open(&wire).unwrap()).unwrap();
    assert_eq!(lab.status()["state"], "idle");
    assert_eq!(lab.json(&["reply", "test/kimi"])["text"], "main reply");
    // A new input must not reuse the previous main turn's completion or reply.
    lab.event(
        "UserPromptSubmit",
        json!({"prompt":"second","t":lab.at(1001.0)}),
    );
    assert_eq!(lab.status()["state"], "working");
    assert!(!lab.out(&["reply", "test/kimi"]).status.success());
}

#[test]
fn wire_reads_only_appends_and_replays_after_file_or_turn_changes() {
    let lab = Lab::new("exec /bin/cat", None);
    let wire = lab.path("wire.jsonl");
    lab.event(
        "SessionStart",
        json!({"transcript_path":wire,"source":"startup"}),
    );
    lab.event(
        "UserPromptSubmit",
        json!({"prompt":"first","t":lab.at(1000.0)}),
    );
    let prompt = json!({"type":"turn.prompt","agentId":"main","turnId":1,"time":lab.ms(1000000)});
    let part = |text: &str| json!({"type":"context.append_loop_event","agentId":"main","time":lab.ms(1000001),"event":{"type":"content.part","turnId":"1","step":0,"part":{"type":"text","text":text}}});
    let end = json!({"type":"turn.ended","agentId":"main","turnId":1,"reason":"completed","time":lab.ms(1000002)});
    let prefix = format!("{prompt}\n{}\n", part("old"));
    fs::write(&wire, &prefix).unwrap();
    assert_eq!(lab.status()["state"], "working");
    // Poison already-consumed bytes without changing identity or length: the next read
    // must only consume the append, retaining the previously accumulated text.
    fs::OpenOptions::new()
        .write(true)
        .open(&wire)
        .unwrap()
        .write_all(prefix.replace("old", "bad").as_bytes())
        .unwrap();
    writeln!(
        fs::OpenOptions::new().append(true).open(&wire).unwrap(),
        "{}\n{end}",
        part(" tail")
    )
    .unwrap();
    assert_eq!(lab.json(&["reply", "test/kimi"])["text"], "old tail");
    // Another query (including a late observational hook) keeps the cached completion.
    lab.event("Kimi:Stop", json!({}));
    assert_eq!(lab.status()["state"], "idle");
    assert_eq!(lab.json(&["reply", "test/kimi"])["text"], "old tail");

    let complete = format!("{prompt}\n{}\n{}\n{end}\n", part("new"), part(" tail"));
    assert_eq!(complete.len() as u64, fs::metadata(&wire).unwrap().len());
    fs::write(lab.path("replacement"), &complete).unwrap();
    fs::rename(lab.path("replacement"), &wire).unwrap();
    assert_eq!(lab.json(&["reply", "test/kimi"])["text"], "new tail");
    // Truncation on the same inode also resets the accumulated result.
    fs::write(&wire, format!("{prompt}\n{}\n{end}\n", part("cut"))).unwrap();
    assert_eq!(lab.json(&["reply", "test/kimi"])["text"], "cut");
    fs::OpenOptions::new()
        .write(true)
        .open(&wire)
        .unwrap()
        .write_all(format!("{prompt}\n{}\n{end}\n", part("now")).as_bytes())
        .unwrap();
    // Even within the same wire millisecond, a changed input timestamp must replay.
    lab.event(
        "UserPromptSubmit",
        json!({"prompt":"second","t":lab.at(1000.0005)}),
    );
    assert_eq!(lab.json(&["reply", "test/kimi"])["text"], "now");
    lab.event(
        "UserPromptSubmit",
        json!({"prompt":"third","t":lab.at(1001.0)}),
    );
    assert_eq!(lab.status()["state"], "working");
    assert!(!lab.out(&["reply", "test/kimi"]).status.success());
}

#[test]
fn prompt_waits_for_the_editor_and_keeps_a_leading_dash() {
    let lab = Lab::new(
        r#"printf 'Loading\n'
printf '%s' "$#" > "$HOME/argc"
sleep 0.2
printf '\033[2J\033[HNo session yet — one will be created on your first message.\n> \nModel   /tmp\ncontext: 0%% (0/1M)\n'
IFS= read -r input
printf '%s' "$input" > "$HOME/received"
printf '%s\n' '{"session_id":"session-test","cwd":"/tmp","source":"startup"}' | "$CORRAL_HELPER" __hook kimi:SessionStart
printf '%s\n' '{"session_id":"session-test","cwd":"/tmp","prompt":[{"type":"text","text":"-first"}]}' | "$CORRAL_HELPER" __hook kimi:UserPromptSubmit
exec /bin/cat"#,
        Some("-first"),
    );
    eventually(|| lab.path("received").exists());
    assert_eq!(fs::read_to_string(lab.path("argc")).unwrap(), "0");
    assert_eq!(fs::read_to_string(lab.path("received")).unwrap(), "-first");
    eventually(|| lab.status()["state"] == "working");
}

#[test]
fn questions_exist_only_while_the_main_question_interaction_is_open() {
    let lab = Lab::new("exec /bin/cat", None);
    let wire = lab.path("wire.jsonl");
    lab.event(
        "SessionStart",
        json!({"transcript_path":wire,"source":"startup"}),
    );
    lab.event(
        "UserPromptSubmit",
        json!({"prompt":"ask","t":lab.at(1000.0)}),
    );
    let append = |value: Value| {
        writeln!(
            fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(&wire)
                .unwrap(),
            "{value}"
        )
        .unwrap();
    };
    append(json!({"type":"turn.prompt","agentId":"main","turnId":1,"time":lab.ms(1000001)}));
    append(
        json!({"type":"context.append_loop_event","agentId":"main","time":lab.ms(1000002),
        "event":{"type":"tool.call","name":"AskUserQuestion","turnId":"1"}}),
    );
    assert!(
        lab.status()["questions"].is_null(),
        "tool call alone is not an open dialog"
    );
    let input = json!([
        {"header":"颜色","question":"选什么颜色？","options":[{"label":"红","description":"暖"},{"label":"蓝","description":"冷"}]},
        {"header":"水果","question":"选哪些？","multiSelect":true,"options":[{"label":"苹果","description":"脆"}],"answers":"not retained"}
    ]);
    let request = json!({"type":"interaction.request","agentId":"main","id":"question-1","kind":"question",
        "request":{"turnId":1,"questions":input},"time":lab.ms(1000003)});
    let expected = json!({"items":[
        {"header":"颜色","question":"选什么颜色？","multi_select":false,"options":[{"label":"红","description":"暖"},{"label":"蓝","description":"冷"}]},
        {"header":"水果","question":"选哪些？","multi_select":true,"options":[{"label":"苹果","description":"脆"}]}
    ],"truncated":false});
    for resolution in [json!({"answers":{"颜色":"红"}}), Value::Null] {
        append(request.clone());
        let st = lab.status();
        assert_eq!(st["state"], "blocked");
        assert_eq!(st["questions"], expected);
        assert_eq!(
            lab.status()["questions"],
            expected,
            "cached reads retain the open question"
        );
        append(
            json!({"type":"interaction.resolved","agentId":"main","id":"question-1","resolution":resolution,"time":lab.ms(1000004)}),
        );
        let st = lab.status();
        assert_eq!(st["state"], "working");
        assert!(
            st["questions"].is_null(),
            "answer and dismissal both close the dialog"
        );
    }
    // A child's prompt must not expose a question on the main agent.
    let mut child = request.clone();
    child["agentId"] = json!("agent-0");
    append(child);
    assert!(lab.status()["questions"].is_null());
    append(request);
    assert_eq!(lab.status()["questions"], expected);
    append(
        json!({"type":"interaction.resolved","agentId":"main","id":"question-1","resolution":null,"time":lab.ms(1000005)}),
    );
    append(
        json!({"type":"turn.ended","agentId":"main","turnId":1,"reason":"cancelled","time":lab.ms(1000006)}),
    );
    let st = lab.status();
    assert_eq!(st["state"], "idle");
    assert_eq!(st["interrupted"], true);
    assert!(st["questions"].is_null());
}

#[test]
fn kimi_questions_use_the_r43_json_budget() {
    let lab = Lab::new("exec /bin/cat", None);
    let wire = lab.path("wire.jsonl");
    lab.event(
        "SessionStart",
        json!({"transcript_path":wire,"source":"startup"}),
    );
    lab.event(
        "UserPromptSubmit",
        json!({"prompt":"ask","t":lab.at(1000.0)}),
    );
    let long = "汉\"\\\n".repeat(5000);
    let questions =
        json!([{"header":"长题","question":long,"options":[{"label":"甲","description":"乙"}]}]);
    let prompt = json!({"type":"turn.prompt","agentId":"main","turnId":1,"time":lab.ms(1000001)});
    let request = json!({"type":"interaction.request","agentId":"main","id":"q","kind":"question",
        "request":{"turnId":1,"questions":questions},"time":lab.ms(1000002)});
    fs::write(&wire, format!("{prompt}\n{request}\n")).unwrap();
    let actual = lab.status()["questions"].clone();
    assert_eq!(actual["truncated"], true);
    assert!(serde_json::to_vec(&actual).unwrap().len() <= 16 * 1024);
    let text = actual["items"][0]["question"].as_str().unwrap();
    assert!(!text.is_empty() && long.starts_with(text));
    // Invoke the existing Claude collector with identical input for exact R43 parity.
    let events = lab.path("claude-events");
    let mut hook = Command::new(lab.path("corral"))
        .args(["__hook", "PermissionRequest"])
        .env("HOME", lab.root.path())
        .env("CORRAL_HOME", lab.path("pens"))
        .env("KIMI_CODE_HOME", lab.path("kimi-home"))
        .env("CORRAL_EVENTS", &events)
        .env("CORRAL_INSTANCE", "synthetic")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    write!(
        hook.stdin.take().unwrap(),
        "{}",
        json!({"tool_name":"AskUserQuestion","tool_input":{"questions":questions}})
    )
    .unwrap();
    assert!(hook.wait().unwrap().success());
    let event: Value = serde_json::from_slice(&fs::read(events).unwrap()).unwrap();
    assert_eq!(actual, event["questions"]);
    append_wire(
        &wire,
        json!({"type":"interaction.resolved","agentId":"main","id":"q","time":lab.ms(1000003),"response":null}),
    );
    let call = json!({"type":"context.append_loop_event","agentId":"main","time":lab.ms(1000004),
        "event":{"type":"tool.call","name":"AskUserQuestion","toolCallId":"bg","args":{"background":true}}});
    let mut background = request;
    background["id"] = json!("bg");
    background["time"] = json!(lab.ms(1000005));
    background["request"]["toolCallId"] = json!("bg");
    writeln!(
        fs::OpenOptions::new().append(true).open(&wire).unwrap(),
        "{call}\n{background}"
    )
    .unwrap();
    let actual = lab.status()["questions"].clone();
    assert_eq!(actual["background"], true);
    assert_eq!(actual["truncated"], true);
    assert!(actual.to_string().len() <= 16 * 1024);
    let text = actual["items"][0]["question"].as_str().unwrap();
    assert!(!text.is_empty() && long.starts_with(text));
}

#[test]
#[ignore = "synthetic subprocess for the stop test"]
fn synthetic_command_in_another_group() {
    unsafe {
        libc::setpgid(0, 0);
        libc::signal(libc::SIGTERM, libc::SIG_IGN);
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
    }
    fs::write(
        PathBuf::from(std::env::var_os("HOME").unwrap()).join("command.pid"),
        std::process::id().to_string(),
    )
    .unwrap();
    std::thread::sleep(Duration::from_secs(60));
}

fn alive(pid: i32) -> bool {
    let out = Command::new("/bin/ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .unwrap();
    let state = String::from_utf8_lossy(&out.stdout);
    !state.trim().is_empty() && !state.trim().starts_with('Z')
}

#[test]
fn stop_waits_for_the_kimi_command_tree_and_leaves_unrelated_processes_alive() {
    let mut lab = Lab::new(
        "\"$HOME/child\" --ignored --exact synthetic_command_in_another_group &\nexec /bin/cat",
        None,
    );
    eventually(|| lab.path("command.pid").exists());
    let child: i32 = fs::read_to_string(lab.path("command.pid"))
        .unwrap()
        .parse()
        .unwrap();
    lab.strays.push(child);
    let instance = lab.status()["instance"].clone();
    let mut unrelated = Command::new("/bin/sleep").arg("60").spawn().unwrap();
    let result = lab.out(&["stop", "test/kimi", "--timeout", "8"]);
    let child_alive = alive(child);
    let unrelated_alive = alive(unrelated.id() as i32);
    unrelated.kill().unwrap();
    unrelated.wait().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert!(!child_alive, "Kimi's command survived stop");
    assert!(unrelated_alive, "stop touched an unrelated process");
    assert_eq!(
        result.stdout,
        format!(
            "{}\n",
            json!({"ok":true,"name":"test/kimi","instance":instance,
                "exit_code":-libc::SIGTERM,"stopped_by":"SIGKILL"})
        )
        .into_bytes()
    );
}

#[test]
fn question_cancel_keeps_working_and_only_cancelled_turns_are_interrupted() {
    let lab = Lab::new("exec /bin/cat", None);
    let wire = lab.path("wire.jsonl");
    lab.event(
        "SessionStart",
        json!({"transcript_path":wire,"source":"startup"}),
    );
    lab.event(
        "UserPromptSubmit",
        json!({"prompt":"ask","t":lab.at(1000.0)}),
    );
    let append = |v: Value| {
        writeln!(
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&wire)
                .unwrap(),
            "{v}"
        )
        .unwrap()
    };
    append(json!({"type":"turn.prompt","agentId":"main","turnId":1,"time":lab.ms(1000001)}));
    append(
        json!({"type":"interaction.request","agentId":"main","id":"question","kind":"question","request":{"turnId":1},"time":lab.ms(1000002)}),
    );
    assert_eq!(lab.status()["state"], "blocked");
    append(
        json!({"type":"interaction.resolved","agentId":"main","id":"question","response":null,"time":lab.ms(1000003)}),
    );
    let st = lab.status();
    assert_eq!(
        (&st["state"], &st["interrupted"]),
        (&json!("working"), &json!(false))
    );
    lab.event("Kimi:Interrupt", json!({}));
    assert_eq!(
        lab.status()["state"],
        "working",
        "hook alone does not identify the main turn"
    );
    append(
        json!({"type":"turn.ended","agentId":"main","turnId":1,"reason":"cancelled","time":lab.ms(1000004)}),
    );
    assert_eq!(
        lab.json(&["wait", "test/kimi", "--timeout", "1"])["result"],
        "interrupted"
    );
    let st = lab.status();
    assert_eq!(
        (&st["state"], &st["interrupted"]),
        (&json!("idle"), &json!(true))
    );
    let reply: Value = serde_json::from_slice(&lab.out(&["reply", "test/kimi"]).stdout).unwrap();
    assert_eq!(
        (&reply["error"], &reply["interrupted"]),
        (&json!("no_reply"), &json!(true))
    );
}

#[test]
fn global_observers_are_silent_and_ignore_inherited_identity_in_a_nested_agent() {
    let lab = Lab::new(
        r#"printf '%s\n' '{"session_id":"session-test","cwd":"/tmp","source":"startup"}' | "$CORRAL_HELPER" __hook kimi:SessionStart > "$HOME/output" 2> "$HOME/errors"
printf '%s\n' '{"session_id":"session-test","cwd":"/tmp","prompt":[{"type":"text","text":"parent"}]}' | "$CORRAL_HELPER" __hook kimi:UserPromptSubmit >> "$HOME/output" 2>> "$HOME/errors"
/bin/sh -c 'printf "%s\n" "{\"session_id\":\"child\",\"cwd\":\"/tmp\",\"source\":\"startup\"}" | "$CORRAL_HELPER" __hook kimi:SessionStart; sleep 0.1' >> "$HOME/output" 2>> "$HOME/errors"
touch "$HOME/finished"
exec /bin/cat"#,
        None,
    );
    eventually(|| lab.path("finished").exists());
    assert_eq!(fs::read(lab.path("output")).unwrap(), b"");
    assert_eq!(fs::read(lab.path("errors")).unwrap(), b"");
    let st = lab.status();
    assert_eq!(st["state"], "working");
    let events = fs::read_to_string(lab.path("pens/test/kimi/events")).unwrap();
    assert!(!events.contains("\"child\""));
    let output = Command::new(lab.path("corral"))
        .args(["__hook", "kimi:Stop"])
        .env_remove("CORRAL_EVENTS")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
}

#[test]
fn screen_fallback_recognizes_approval_and_question_panels() {
    let lab = Lab::new(
        r#"printf '▶ Run this command?\n$ true\n▶ 1. Approve once\n2. Approve for this session\n3. Reject\n4. Reject with feedback\n↑/↓ select · 1/2/3/4 choose · ↵ confirm\n'
read choice
printf '\033[2J\033[Hquestion\nTEST Submit\n? Pick one\n→ [1] A\n[2] B\n[3] Other\n↑↓ select  1-3 / ↵ choose  ←/→/tab switch  esc cancel\n'
read choice
printf '\033[2J\033[HWorking\n'
exec /bin/cat"#,
        None,
    );
    lab.event("SessionStart", json!({"source":"startup"}));
    lab.event("UserPromptSubmit", json!({"prompt":"run"}));
    eventually(|| lab.status()["state"] == "blocked");
    lab.json(&["keys", "test/kimi", "enter"]);
    eventually(|| {
        String::from_utf8_lossy(&lab.out(&["read", "test/kimi"]).stdout).contains("esc cancel")
    });
    assert_eq!(lab.status()["state"], "blocked");
    lab.json(&["keys", "test/kimi", "enter"]);
    eventually(|| lab.status()["state"] == "working");
}
