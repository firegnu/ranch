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
}
impl Lab {
    fn new(script: &str, prompt: Option<&str>) -> Self {
        let lab = Self {
            root: tempfile::tempdir_in("/tmp").unwrap(),
            strays: vec![],
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
        lab
    }
    fn path(&self, name: &str) -> PathBuf {
        self.root.path().join(name)
    }
    fn out(&self, args: &[&str]) -> Output {
        Command::new(self.path("corral"))
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
    lab.event("UserPromptSubmit", json!({"prompt":"first","t":1000.0}));
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
    append(
        json!({"type":"turn.prompt","agentId":"main","turnId":0,"time":1,"input":[{"type":"text","text":"old"}]}),
    );
    append(json!({"type":"turn.ended","agentId":"main","turnId":0,"time":2,"reason":"completed"}));
    append(
        json!({"type":"turn.prompt","agentId":"main","turnId":1,"time":1000001,"input":[{"type":"text","text":"first"}]}),
    );
    append(
        json!({"type":"context.append_loop_event","agentId":"main","time":1000002,"event":{"type":"tool.call","name":"Bash","turnId":"1"}}),
    );
    let st = lab.status();
    assert_eq!(
        (&st["state"], &st["last_tool"]),
        (&json!("working"), &json!("Bash"))
    );
    append(
        json!({"type":"interaction.request","agentId":"main","id":"approval","kind":"approval","request":{"turnId":1,"toolName":"Bash"},"time":1000003}),
    );
    assert_eq!(lab.status()["state"], "blocked");
    append(json!({"type":"interaction.resolved","agentId":"main","id":"approval","time":1000004}));
    assert_eq!(lab.status()["state"], "working");
    append(
        json!({"type":"context.append_loop_event","agentId":"main","time":1000005,"event":{"type":"content.part","turnId":"1","part":{"type":"think","text":"hidden"}}}),
    );
    append(
        json!({"type":"context.append_loop_event","agentId":"main","time":1000006,"event":{"type":"content.part","turnId":"1","part":{"type":"text","text":"main "}}}),
    );
    // A child's event and the indistinguishable Stop hook cannot close the main turn.
    append(
        json!({"type":"turn.ended","agentId":"agent-0","turnId":0,"reason":"completed","time":1000007}),
    );
    lab.event("Kimi:Stop", json!({}));
    assert_eq!(lab.status()["state"], "working");
    assert!(!lab.out(&["reply", "test/kimi"]).status.success());
    append(
        json!({"type":"context.append_loop_event","agentId":"main","time":1000008,"event":{"type":"content.part","turnId":"1","part":{"type":"text","text":"reply"}}}),
    );
    let end = json!({"type":"turn.ended","agentId":"main","turnId":1,"reason":"completed","time":1000009});
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
    lab.event("UserPromptSubmit", json!({"prompt":"second","t":1001.0}));
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
}

#[test]
fn question_cancel_keeps_working_and_only_cancelled_turns_are_interrupted() {
    let lab = Lab::new("exec /bin/cat", None);
    let wire = lab.path("wire.jsonl");
    lab.event(
        "SessionStart",
        json!({"transcript_path":wire,"source":"startup"}),
    );
    lab.event("UserPromptSubmit", json!({"prompt":"ask","t":1000.0}));
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
    append(json!({"type":"turn.prompt","agentId":"main","turnId":1,"time":1000001}));
    append(
        json!({"type":"interaction.request","agentId":"main","id":"question","kind":"question","request":{"turnId":1},"time":1000002}),
    );
    assert_eq!(lab.status()["state"], "blocked");
    append(
        json!({"type":"interaction.resolved","agentId":"main","id":"question","response":null,"time":1000003}),
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
        json!({"type":"turn.ended","agentId":"main","turnId":1,"reason":"cancelled","time":1000004}),
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
