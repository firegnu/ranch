//! Per-agent behaviour seen through the public CLI, pinned by snapshots taken before the adapter
//! refactor (R3). The snapshot files must not change; regenerate only with CORRAL_WRITE_SNAPSHOTS=1
//! on code whose behaviour is meant to change.
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{Duration, Instant},
};

fn snapshot(name: &str, value: &Value) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(name);
    let text = serde_json::to_string_pretty(value).unwrap() + "\n";
    if std::env::var_os("CORRAL_WRITE_SNAPSHOTS").is_some() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &text).unwrap();
        return;
    }
    let expected = fs::read_to_string(&path).unwrap();
    if expected != text {
        let actual = path.with_extension("actual.json");
        fs::write(&actual, &text).unwrap();
        panic!("{name} differs from its snapshot; see {}", actual.display());
    }
}

struct Lab {
    root: tempfile::TempDir,
    core: PathBuf,
    names: std::cell::RefCell<Vec<String>>,
}
impl Lab {
    fn new() -> Self {
        let root = tempfile::tempdir_in("/tmp").unwrap();
        let core = root.path().join("corral");
        fs::copy(env!("CARGO_BIN_EXE_corral"), &core).unwrap();
        fs::create_dir(root.path().join("bin")).unwrap();
        Self {
            root,
            core,
            names: Default::default(),
        }
    }
    fn cmd(&self) -> Command {
        let mut c = Command::new(&self.core);
        c.env("HOME", self.root.path())
            .env("CORRAL_HOME", self.root.path().join("pens"))
            .env("SHELL", "/bin/sh")
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.root.path().join("bin").display()),
            )
            .env_remove("CLAUDE_CONFIG_DIR")
            .env_remove("CODEX_SANDBOX");
        c
    }
    /// Exit code and JSON output, with the lab's temporary paths replaced by placeholders.
    fn result(&self, o: Output) -> Value {
        let out: Value = serde_json::from_slice(&o.stdout)
            .unwrap_or_else(|_| json!({"stdout": String::from_utf8_lossy(&o.stdout)}));
        json!({"code": o.status.code(), "out": self.normal(out)})
    }
    fn normal(&self, v: Value) -> Value {
        let root = self.root.path().to_str().unwrap().to_owned();
        let real = fs::canonicalize(self.root.path())
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        match v {
            Value::String(s) => Value::String(s.replace(&real, "<ROOT>").replace(&root, "<ROOT>")),
            Value::Array(a) => Value::Array(a.into_iter().map(|v| self.normal(v)).collect()),
            Value::Object(m) => {
                Value::Object(m.into_iter().map(|(k, v)| (k, self.normal(v))).collect())
            }
            v => v,
        }
    }
    /// A fake agent that records the argv it was started with, then behaves like `cat`.
    fn program(&self, name: &str) -> PathBuf {
        let path = self.root.path().join("bin").join(name);
        fs::write(
            &path,
            "#!/bin/sh\nprintf '%s\\0' \"$0\" \"$@\" > \"$CORRAL_EVENTS.args\"\nexec /bin/cat\n",
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }
    fn start(&self, name: &str, program: &str, prompt: Option<&str>, args: &[&str]) -> Value {
        let mut c = self.cmd();
        c.args(["start", name, "--cwd", "/tmp", "--env"])
            .arg(format!(
                "PATH={}:/usr/bin:/bin",
                self.root.path().join("bin").display()
            ));
        if let Some(p) = prompt {
            c.args(["--prompt", p]);
        }
        c.arg("--").arg(program).args(args);
        let started = self.result(c.output().unwrap());
        if started["code"] != 0 {
            return json!({"start": started});
        }
        self.names.borrow_mut().push(name.to_owned());
        let dir = self.root.path().join("pens").join(name);
        let args_file = dir.join("events.args");
        let end = Instant::now() + Duration::from_secs(5);
        while !args_file.exists() {
            assert!(Instant::now() < end, "{name} never started its program");
            std::thread::sleep(Duration::from_millis(20));
        }
        std::thread::sleep(Duration::from_millis(50));
        // Every argument ends with a NUL, so only the piece after the last one is not an argument.
        let bytes = fs::read(&args_file).unwrap();
        let argv: Vec<Value> = bytes[..bytes.len() - 1]
            .split(|c| *c == 0)
            .map(|b| json!(std::str::from_utf8(b).unwrap()))
            .collect();
        let mut hooks = json!({});
        for file in ["hook.py", "hook_pi.ts", "hook_omp.ts"] {
            let path = dir.join(file);
            if let Ok(body) = fs::read_to_string(&path) {
                let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
                hooks[file] = json!({"mode": format!("{mode:o}"), "body": body});
            }
        }
        let status = self.result(self.cmd().args(["status", name]).output().unwrap());
        let stopped = self.result(
            self.cmd()
                .args(["stop", name, "--timeout", "20"])
                .output()
                .unwrap(),
        );
        let pick = |v: &Value, keys: &[&str]| {
            let mut out = json!({"code": v["code"]});
            for k in keys {
                out[*k] = v["out"][*k].clone();
            }
            out
        };
        json!({
            "start": pick(&started, &["ok", "name", "kind"]),
            "argv": self.normal(Value::Array(argv)),
            "hook_files": hooks,
            "status": pick(&status, &["ok", "kind", "state"]),
            "stop": pick(&stopped, &["ok", "exit_code", "stopped_by"]),
        })
    }
}
impl Drop for Lab {
    fn drop(&mut self) {
        for name in self.names.borrow().iter() {
            let _ = self.cmd().args(["stop", name, "--timeout", "10"]).output();
        }
    }
}

#[test]
fn each_kind_starts_reports_and_stops_as_before() {
    let lab = Lab::new();
    let mut cases = json!({});
    for kind in ["claude", "codex", "pi", "omp", "other", "claude-code"] {
        let program = lab.program(kind);
        let program = program.to_str().unwrap();
        cases[format!("{kind}/plain")] =
            lab.start(&format!("lab/{kind}-plain"), program, None, &[]);
        cases[format!("{kind}/args")] = lab.start(
            &format!("lab/{kind}-args"),
            program,
            None,
            &["--model", "fixture", "--", "positional"],
        );
        cases[format!("{kind}/prompt")] = lab.start(
            &format!("lab/{kind}-prompt"),
            program,
            Some("-literal prompt"),
            &["--model", "fixture"],
        );
        cases[format!("{kind}/empty-prompt")] =
            lab.start(&format!("lab/{kind}-empty"), program, Some(""), &[]);
    }
    // Found on PATH by bare name: the kind is still the program's file name.
    for kind in ["claude", "codex", "pi", "omp"] {
        cases[format!("{kind}/bare")] =
            lab.start(&format!("lab/{kind}-bare"), kind, Some("hi"), &["-x"]);
    }
    snapshot("adapters_start.json", &cases);
}

#[test]
fn skills_install_where_they_did() {
    let lab = Lab::new();
    let project = lab.root.path().join("project");
    fs::create_dir(&project).unwrap();
    let project = project.to_str().unwrap();
    let config = lab.root.path().join("claude-config");
    let config = config.to_str().unwrap();
    let run = |args: &[&str], claude_config: Option<&str>| {
        let mut c = lab.cmd();
        c.arg("install-skills").args(args);
        if let Some(dir) = claude_config {
            c.env("CLAUDE_CONFIG_DIR", dir);
        }
        lab.result(c.output().unwrap())
    };
    let mut cases = json!({});
    for target in [None, Some("all"), Some("claude"), Some("codex")] {
        let mut args = vec!["--dry-run"];
        if let Some(t) = target {
            args.extend(["--target", t]);
        }
        let key = target.unwrap_or("default");
        cases[format!("{key}/home")] = run(&args, None);
        cases[format!("{key}/claude-config")] = run(&args, Some(config));
        cases[format!("{key}/empty-claude-config")] = run(&args, Some(""));
        let mut with_project = args.clone();
        with_project.extend(["--project", project]);
        cases[format!("{key}/project")] = run(&with_project, Some(config));
        let mut remove = args.clone();
        remove.push("--remove");
        cases[format!("{key}/remove")] = run(&remove, None);
    }
    for target in ["pi", "omp", "Claude", ""] {
        cases[format!("bad-target/{target}")] = run(&["--dry-run", "--target", target], None);
    }
    cases["bad-project"] = run(
        &["--dry-run", "--project", &format!("{project}/missing")],
        None,
    );
    snapshot("adapters_skills.json", &cases);
}

/// amp (R6) is new, so it is checked directly rather than against the R3 snapshots.
#[test]
fn amp_starts_bare_and_reads_the_events_its_plugin_writes() {
    let lab = Lab::new();
    // Records its environment as well as its argv.
    let fake = "#!/bin/sh\nenv > \"$CORRAL_EVENTS.env\"\nprintf '%s\\0' \"$0\" \"$@\" > \"$CORRAL_EVENTS.args\"\nexec /bin/cat\n";
    for name in ["amp", "claude"] {
        let path = lab.root.path().join("bin").join(name);
        fs::write(&path, fake).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let env = |name: &str| -> Vec<String> {
        let text =
            fs::read_to_string(lab.root.path().join("pens").join(name).join("events.env")).unwrap();
        text.lines()
            .filter(|l| l.starts_with("CORRAL_"))
            .map(|l| l.split('=').next().unwrap().to_owned())
            .collect()
    };
    let run = |name: &str, program: &str, prompt: Option<&str>| {
        let mut c = lab.cmd();
        c.args(["start", name, "--cwd", "/tmp", "--env"])
            .arg(format!(
                "PATH={}:/usr/bin:/bin",
                lab.root.path().join("bin").display()
            ));
        if let Some(p) = prompt {
            c.args(["--prompt", p]);
        }
        let o = lab.result(c.arg("--").arg(program).output().unwrap());
        if o["code"] == 0 {
            lab.names.borrow_mut().push(name.to_owned());
            let args = lab.root.path().join("pens").join(name).join("events.args");
            let end = Instant::now() + Duration::from_secs(5);
            while !args.exists() {
                assert!(Instant::now() < end, "{name} never started its program");
                std::thread::sleep(Duration::from_millis(20));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        o
    };

    let o = run("lab/amp-prompt", "amp", Some("hi"));
    assert_eq!(
        o,
        json!({"code":1,"out":{"ok":false,"error":"usage","message":"amp takes no --prompt"}})
    );

    assert_eq!(run("lab/amp", "amp", None)["code"], 0);
    let dir = lab.root.path().join("pens/lab/amp");
    let args = fs::read(dir.join("events.args")).unwrap();
    assert_eq!(
        args,
        format!("{}/bin/amp\0", lab.root.path().display()).into_bytes()
    );
    let mut names = env("lab/amp");
    names.sort();
    assert_eq!(
        names,
        [
            "CORRAL_CWD",
            "CORRAL_EVENTS",
            "CORRAL_HOME",
            "CORRAL_INSTANCE",
            "CORRAL_KIND",
            "CORRAL_NAME",
            "CORRAL_PEN_PID"
        ]
    );
    let text = fs::read_to_string(dir.join("events.env")).unwrap();
    assert!(text.lines().any(|l| l == "CORRAL_KIND=amp"), "{text}");
    assert!(text.lines().any(|l| l == "CORRAL_CWD=/tmp"), "{text}");
    // The plugin checks that its amp is a child of this pen.
    let o = lab.cmd().args(["status", "lab/amp"]).output().unwrap();
    let s: Value = serde_json::from_slice(&o.stdout).unwrap();
    let pen = format!("CORRAL_PEN_PID={}", s["pen_pid"]);
    assert!(text.lines().any(|l| l == pen), "{text}");
    // Agents whose hooks come with their start get neither.
    assert_eq!(run("lab/claude", "claude", None)["code"], 0);
    let mut names = env("lab/claude");
    names.sort();
    assert_eq!(
        names,
        [
            "CORRAL_EVENTS",
            "CORRAL_HOME",
            "CORRAL_INSTANCE",
            "CORRAL_NAME"
        ]
    );

    // The lines the plugin writes, in the order a turn with one approval produces them.
    let inst = text
        .lines()
        .find_map(|l| l.strip_prefix("CORRAL_INSTANCE="))
        .unwrap()
        .to_owned();
    let state = |events: &[Value]| {
        let mut f = fs::OpenOptions::new()
            .append(true)
            .open(dir.join("events"))
            .unwrap();
        for e in events {
            let mut e = e.clone();
            for (k, v) in [
                ("v", json!(1)),
                ("inst", json!(inst)),
                ("session_id", json!("T-main")),
                ("cwd", json!("/tmp")),
                ("has_transcript", json!(true)),
                ("version", json!(1)),
            ] {
                if e.get(k).is_none() {
                    e[k] = v;
                }
            }
            use std::io::Write;
            writeln!(f, "{e}").unwrap();
        }
        let s = lab.result(lab.cmd().args(["status", "lab/amp"]).output().unwrap());
        (s["out"]["state"].clone(), s["out"]["kind"].clone())
    };
    assert_eq!(
        state(&[json!({"t":1.0,"ev":"SessionStart","source":"startup"})]),
        (json!("idle"), json!("amp"))
    );
    assert_eq!(
        state(&[json!({"t":2.0,"ev":"UserPromptSubmit","prompt":"hello"})]).0,
        "working"
    );
    assert_eq!(
        state(&[json!({"t":3.0,"ev":"Notification","notification_type":"permission_prompt"})]).0,
        "blocked"
    );
    assert_eq!(state(&[json!({"t":4.0,"ev":"PostToolUse"})]).0, "working");
    assert_eq!(
        state(&[json!({"t":5.0,"ev":"PostToolUse","tool_name":"Bash"})]).0,
        "working"
    );
    // amp shows a pending approval only in the terminal title; the fake echoes it back.
    let title = |t: &str| {
        let keys = [format!("text:\x1b]0;{t}\x07"), "enter".into()];
        let o = lab
            .cmd()
            .args(["keys", "lab/amp"])
            .args(&keys)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            let s = lab.result(lab.cmd().args(["status", "lab/amp"]).output().unwrap());
            if s["out"]["title"] == t {
                return s["out"]["state"].clone();
            }
            assert!(Instant::now() < end, "title never became {t}: {s}");
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    assert_eq!(
        title("◆ Input needed: Create a file - amp - /tmp"),
        "blocked"
    );
    assert_eq!(title("Create a file - amp - /tmp"), "working");
    // A subagent's thread never became the main session, so it changes nothing.
    assert_eq!(
        state(&[json!({"t":5.5,"ev":"Stop","session_id":"T-sub","last_assistant_message":"sub"})])
            .0,
        "working"
    );
    assert_eq!(
        state(&[json!({"t":6.0,"ev":"Stop","last_assistant_message":"done here"})]).0,
        "idle"
    );
    let reply = lab.result(lab.cmd().args(["reply", "lab/amp"]).output().unwrap());
    assert_eq!(reply["out"]["text"], "done here", "{reply}");
    // A turn whose start amp did not report still shows its approval in the title.
    assert_eq!(title("◆ Input needed: Next - amp - /tmp"), "blocked");
    assert_eq!(title("Next - amp - /tmp"), "idle");
    assert_eq!(
        state(&[json!({"t":7.0,"ev":"UserPromptSubmit","prompt":"again"})]).0,
        "working"
    );
    assert_eq!(state(&[json!({"t":8.0,"ev":"Interrupt"})]).0, "idle");
    assert_eq!(
        state(&[
            json!({"t":9.0,"ev":"UserPromptSubmit","prompt":"x"}),
            json!({"t":10.0,"ev":"StopFailure"})
        ])
        .0,
        "idle"
    );

    let stopped = lab.result(
        lab.cmd()
            .args(["stop", "lab/amp", "--timeout", "20"])
            .output()
            .unwrap(),
    );
    assert_eq!(
        (&stopped["out"]["ok"], &stopped["out"]["stopped_by"]),
        (&json!(true), &json!("SIGHUP"))
    );
}

#[test]
fn agents_without_screen_rules_get_no_screen() {
    // R7: a pen keeps a screen only for an adapter with screen rules; of these, none has any (agy,
    // which has, is checked in its own test).
    use std::io::{BufRead, BufReader, Write};
    let lab = Lab::new();
    for kind in ["claude", "codex", "pi", "omp", "amp", "other"] {
        let path = lab.root.path().join("bin").join(kind);
        fs::write(&path, "#!/bin/sh\nprintf 'Allow?\\n'\nexec /bin/cat\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let name = format!("lab/{kind}");
        let o = lab.result(
            lab.cmd()
                .args(["start", &name, "--cwd", "/tmp", "--env"])
                .arg(format!(
                    "PATH={}:/usr/bin:/bin",
                    lab.root.path().join("bin").display()
                ))
                .args(["--", kind])
                .output()
                .unwrap(),
        );
        assert_eq!(o["code"], 0, "{o}");
        lab.names.borrow_mut().push(name.clone());
        let sock = lab.root.path().join("pens").join(&name).join("sock");
        let mut s = std::os::unix::net::UnixStream::connect(sock).unwrap();
        writeln!(s, "{}", json!({"proto":1,"op":"status"})).unwrap();
        let mut line = String::new();
        BufReader::new(&s).read_line(&mut line).unwrap();
        let reply: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(reply["ok"], true, "{reply}");
        assert!(reply.get("screen").is_none(), "{kind}: {reply}");
    }
}

/// agy (R8) records through `__hook agy:<event>`, run by the plugin `integration install` writes.
/// Its hook input carries no prompt or reply; both come from the transcript it names.
#[test]
fn agy_hooks_translate_its_events_only_for_the_agy_its_pen_started() {
    let lab = Lab::new();
    let events = lab.root.path().join("events");
    let transcript = lab.root.path().join("transcript.jsonl");
    let steps = [
        json!({"type":"USER_INPUT","content":"<USER_REQUEST>\nfirst\n</USER_REQUEST>\n<ADDITIONAL_METADATA>\nnow\n</ADDITIONAL_METADATA>"}),
        json!({"type":"PLANNER_RESPONSE","content":"old reply"}),
        json!({"type":"USER_INPUT","content":"<USER_REQUEST>\n  second\n  line\n</USER_REQUEST>"}),
        json!({"type":"PLANNER_RESPONSE","content":"looking"}),
        json!({"type":"TOOL_RESULT","content":"ls output"}),
        json!({"type":"PLANNER_RESPONSE","content":"the answer"}),
        json!({"type":"PLANNER_RESPONSE","content":""}),
    ];
    let write = |steps: &[Value]| {
        let text: String = steps.iter().map(|s| format!("{s}\n")).collect();
        fs::write(&transcript, text).unwrap();
    };
    write(&steps);
    // The hook runs as the agent's child, so the pen is its grandparent: here the test process
    // plays the agent and its parent the pen.
    let pen = unsafe { libc::getppid() }.to_string();
    let none: Vec<Value> = vec![];
    let hook = |event: &str, input: Value, env: &[(&str, &str)]| -> Vec<Value> {
        let _ = fs::remove_file(&events);
        let mut c = Command::new(&lab.core);
        c.args(["__hook", event])
            .env_clear()
            .env("CORRAL_EVENTS", &events)
            .env("CORRAL_INSTANCE", "i-1")
            .env("CORRAL_KIND", "agy")
            .env("CORRAL_CWD", "/work")
            .env("CORRAL_PEN_PID", &pen)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped());
        for (k, v) in env {
            if v.is_empty() {
                c.env_remove(k);
            } else {
                c.env(k, v);
            }
        }
        let mut child = c.spawn().unwrap();
        use std::io::Write;
        write!(child.stdin.take().unwrap(), "{input}").unwrap();
        let o = child.wait_with_output().unwrap();
        assert!(o.status.success(), "{event}");
        // agy needs JSON on stdout from every hook.
        assert_eq!(String::from_utf8_lossy(&o.stdout), "{}\n", "{event}");
        fs::read_to_string(&events)
            .unwrap_or_default()
            .lines()
            .map(|l| {
                let mut v: Value = serde_json::from_str(l).unwrap();
                assert!(v["t"].is_f64(), "{v}");
                v.as_object_mut().unwrap().remove("t");
                v
            })
            .collect()
    };
    let input = |extra: Value| {
        let mut v =
            json!({"conversationId":"c-1","transcriptPath":transcript,"workspacePaths":["/work"]});
        v.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        v
    };
    let record = |fields: Value| {
        let mut v =
            json!({"v":1,"inst":"i-1","cwd":"/work","session_id":"c-1","has_transcript":true});
        v.as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        vec![v]
    };

    assert_eq!(
        hook("agy:SessionStart", input(json!({})), &[]),
        record(json!({"ev":"SessionStart","source":"startup"}))
    );
    // Only the first model call of a turn starts it.
    assert_eq!(
        hook(
            "agy:PreInvocation",
            input(json!({"invocationNum":0,"initialNumSteps":2})),
            &[]
        ),
        record(json!({"ev":"UserPromptSubmit","prompt":"second\n  line"}))
    );
    assert_eq!(
        hook("agy:PreInvocation", input(json!({"invocationNum":1})), &[]),
        none
    );
    assert_eq!(
        hook(
            "agy:PostToolUse",
            input(json!({"toolCall":{"name":"run_command","args":{}}})),
            &[]
        ),
        record(json!({"ev":"PostToolUse","tool_name":"run_command"}))
    );
    // The reply is the last text since the user's last input.
    assert_eq!(
        hook(
            "agy:Stop",
            input(json!({"terminationReason":"NO_TOOL_CALL","fullyIdle":true})),
            &[]
        ),
        record(json!({"ev":"Stop","last_assistant_message":"the answer"}))
    );
    assert_eq!(
        hook(
            "agy:Stop",
            input(json!({"terminationReason":"USER_CANCELED"})),
            &[]
        ),
        record(json!({"ev":"Interrupt"}))
    );
    assert_eq!(
        hook("agy:Stop", input(json!({"terminationReason":"ERROR"})), &[]),
        record(json!({"ev":"StopFailure"}))
    );
    // A turn that has not answered yet has no reply, and an older one is not taken for it.
    write(&steps[..3]);
    assert_eq!(
        hook(
            "agy:Stop",
            input(json!({"terminationReason":"NO_TOOL_CALL"})),
            &[]
        ),
        record(json!({"ev":"Stop","last_assistant_message":null}))
    );
    // Without a transcript there is no prompt, rather than an empty or made-up one.
    let missing = json!({"conversationId":"c-1","transcriptPath":lab.root.path().join("none"),"invocationNum":0});
    assert_eq!(
        hook("agy:PreInvocation", missing, &[]),
        record(json!({"ev":"UserPromptSubmit"}))
    );
    assert_eq!(
        hook("agy:PostInvocation", input(json!({"invocationNum":0})), &[]),
        none
    );

    // Anywhere but the agy its pen started, the hook only answers.
    let stop = input(json!({"terminationReason":"NO_TOOL_CALL"}));
    for env in [
        &[("CORRAL_KIND", "amp")][..],
        &[("CORRAL_KIND", "")],
        &[("CORRAL_PEN_PID", "1")],
        &[("CORRAL_PEN_PID", "")],
        &[("CORRAL_EVENTS", "")],
    ] {
        assert_eq!(hook("agy:Stop", stop.clone(), env), none, "{env:?}");
        assert!(!events.exists(), "{env:?}");
    }
    assert_eq!(hook("nope:Stop", stop.clone(), &[]), none);
    assert_eq!(hook("amp:Stop", stop, &[("CORRAL_KIND", "amp")]), none);
}

/// A fake agy that runs the commands of the plugin corral installs, the way agy runs hooks: as its
/// own children, with its environment.
const FAKE_AGY: &str = r#"#!/bin/sh
env > "$CORRAL_EVENTS.env"
hook() { sh -c "$(cat "$LAB/cmd.$1")" < "$LAB/in.$1" >> "$LAB/out"; }
prompt() {
  printf '{"type":"USER_INPUT","content":"<USER_REQUEST>\\n%s\\n</USER_REQUEST>"}\n' "$1" >> "$LAB/transcript"
  hook PreInvocation
}
printf '%s\0' "$0" "$@" > "$CORRAL_EVENTS.args"
# Like agy, it starts the session with the first prompt.
case $1 in --prompt-interactive=*) hook SessionStart; prompt "${1#--prompt-interactive=}" ;; esac
while IFS= read -r line; do
  case $line in
  ask) printf 'Run ls?\n> Allow\n  Deny\n\342\206\221/\342\206\223 Navigate \302\267 tab Amend \302\267 ctrl+g edit\n' ;;
  tool) printf 'ran\nls\nok\ndone\n'; hook PostToolUse ;;
  nested) (hook Stop; :) ;;
  stop) hook Stop ;;
  *) prompt "$line" ;;
  esac
done
"#;

#[test]
fn agy_starts_bare_and_its_installed_plugin_drives_its_state() {
    use std::io::{BufRead, BufReader, Write};
    let lab = Lab::new();
    let root = lab.root.path();
    let dir = root.join("pens/lab/agy");
    let state_dir = root.join("agy");
    fs::create_dir(&state_dir).unwrap();
    let o = lab.result(
        lab.cmd()
            .args(["integration", "install", "--agent", "agy", "--yes"])
            .output()
            .unwrap(),
    );
    assert_eq!(o["out"]["written"], true, "{o}");
    let hooks: Value = serde_json::from_str(
        &fs::read_to_string(root.join(".gemini/config/plugins/corral/hooks.json")).unwrap(),
    )
    .unwrap();
    let transcript = state_dir.join("transcript");
    for (event, command, input) in [
        (
            "SessionStart",
            &hooks["corral"]["SessionStart"][0]["command"],
            json!({}),
        ),
        (
            "PreInvocation",
            &hooks["corral"]["PreInvocation"][0]["command"],
            json!({"invocationNum":0}),
        ),
        (
            "PostToolUse",
            &hooks["corral"]["PostToolUse"][0]["hooks"][0]["command"],
            json!({"toolCall":{"name":"run_command"}}),
        ),
        (
            "Stop",
            &hooks["corral"]["Stop"][0]["command"],
            json!({"terminationReason":"NO_TOOL_CALL"}),
        ),
    ] {
        fs::write(
            state_dir.join(format!("cmd.{event}")),
            command.as_str().unwrap(),
        )
        .unwrap();
        let mut input = input;
        input["conversationId"] = json!("c-1");
        input["transcriptPath"] = json!(transcript);
        fs::write(state_dir.join(format!("in.{event}")), input.to_string()).unwrap();
    }
    let path = root.join("bin/agy");
    fs::write(&path, FAKE_AGY).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();

    let o = lab.result(
        lab.cmd()
            .args(["start", "lab/agy", "--cwd", "/tmp", "--env"])
            .arg(format!("PATH={}:/usr/bin:/bin", root.join("bin").display()))
            .arg("--env")
            .arg(format!("LAB={}", state_dir.display()))
            .args(["--prompt", "-v and more", "--", "agy"])
            .output()
            .unwrap(),
    );
    assert_eq!(o["code"], 0, "{o}");
    lab.names.borrow_mut().push("lab/agy".into());
    let end = Instant::now() + Duration::from_secs(5);
    while !dir.join("events.args").exists() {
        assert!(Instant::now() < end, "agy never started");
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(50));
    // The first prompt goes in one argument, so it cannot be taken for an option.
    assert_eq!(
        fs::read(dir.join("events.args")).unwrap(),
        format!(
            "{}/bin/agy\0--prompt-interactive=-v and more\0",
            root.display()
        )
        .into_bytes()
    );
    let env = fs::read_to_string(dir.join("events.env")).unwrap();
    for line in ["CORRAL_KIND=agy", "CORRAL_CWD=/tmp"] {
        assert!(env.lines().any(|l| l == line), "{env}");
    }
    let status =
        || lab.result(lab.cmd().args(["status", "lab/agy"]).output().unwrap())["out"].clone();
    let pen = format!("CORRAL_PEN_PID={}", status()["pen_pid"]);
    assert!(env.lines().any(|l| l == pen), "{env}");
    // Its pen keeps a screen.
    let mut s = std::os::unix::net::UnixStream::connect(dir.join("sock")).unwrap();
    writeln!(s, "{}", json!({"proto":1,"op":"status"})).unwrap();
    let mut line = String::new();
    BufReader::new(&s).read_line(&mut line).unwrap();
    let reply: Value = serde_json::from_str(&line).unwrap();
    assert!(reply["screen"].is_string(), "{reply}");

    let keys = |text: &str| {
        let o = lab
            .cmd()
            .args(["keys", "lab/agy", &format!("text:{text}"), "enter"])
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    };
    let wait = |want: &str| {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            let s = status();
            if s["state"] == want {
                return s;
            }
            assert!(Instant::now() < end, "never became {want}: {s}");
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    let answered = || {
        fs::read_to_string(state_dir.join("out"))
            .unwrap_or_default()
            .lines()
            .inspect(|l| assert_eq!(*l, "{}"))
            .count()
    };
    let reply = |text: &str| {
        let mut f = fs::OpenOptions::new()
            .append(true)
            .open(&transcript)
            .unwrap();
        writeln!(f, "{}", json!({"type":"PLANNER_RESPONSE","content":text})).unwrap();
        keys("stop");
        wait("idle");
        let o = lab.result(lab.cmd().args(["reply", "lab/agy"]).output().unwrap());
        assert_eq!(o["out"]["text"], text, "{o}");
    };
    // The prompt the hook reports is the one corral passed: its digest matches.
    let s = wait("working");
    assert_eq!(s["last_input_source"], "send", "{s}");
    reply("hello");
    // What send types reaches the transcript, and the hook reports it as the prompt.
    let sent = lab.result(
        lab.cmd()
            .args(["send", "lab/agy", "list the files"])
            .output()
            .unwrap(),
    );
    assert_eq!(sent["out"]["confirmed"], true, "{sent}");
    let s = wait("working");
    assert_eq!(s["last_input_source"], "send", "{s}");
    // Only its screen shows that agy waits for approval.
    keys("ask");
    wait("blocked");
    // The approval leaves the screen before the tool reports.
    keys("tool");
    let end = Instant::now() + Duration::from_secs(10);
    while status()["last_tool"] != "run_command" {
        assert!(Instant::now() < end, "the tool never reported");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(status()["state"], "working");
    // A hook of an agy started inside this one records nothing.
    let before = fs::read(dir.join("events")).unwrap();
    let count = answered();
    keys("nested");
    let end = Instant::now() + Duration::from_secs(5);
    while answered() == count {
        assert!(Instant::now() < end, "the nested hook never ran");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(fs::read(dir.join("events")).unwrap(), before);
    assert_eq!(status()["state"], "working");
    reply("three files");

    let stopped = lab.result(
        lab.cmd()
            .args(["stop", "lab/agy", "--timeout", "20"])
            .output()
            .unwrap(),
    );
    assert_eq!(
        (&stopped["out"]["ok"], &stopped["out"]["stopped_by"]),
        (&json!(true), &json!("SIGHUP"))
    );
}

#[test]
fn agy_reads_corral_skills_from_its_own_config() {
    let lab = Lab::new();
    let project = lab.root.path().join("project");
    fs::create_dir(&project).unwrap();
    let run =
        |args: &[&str]| lab.result(lab.cmd().arg("install-skills").args(args).output().unwrap());
    let o = run(&["--target", "gemini", "--yes"]);
    assert_eq!(
        o["out"]["items"],
        json!([{"agent":"gemini","path":"<ROOT>/.gemini/config/skills/corral/SKILL.md","status":"create"}])
    );
    assert_eq!(
        fs::read_to_string(
            lab.root
                .path()
                .join(".gemini/config/skills/corral/SKILL.md")
        )
        .unwrap(),
        include_str!("../resources/SKILL.md")
    );
    // In a project agy reads `.agents`, the codex place, so gemini has none there.
    let project = project.to_str().unwrap();
    let o = run(&["--target", "gemini", "--project", project, "--dry-run"]);
    assert_eq!(
        (&o["code"], &o["out"]["error"]),
        (&json!(1), &json!("usage"))
    );
    let o = run(&["--project", project, "--dry-run"]);
    let agents: Vec<_> = o["out"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["agent"].clone())
        .collect();
    assert_eq!(agents, [json!("claude"), json!("codex")]);
    let o = run(&["--target", "gemini", "--remove", "--yes"]);
    assert_eq!(o["out"]["items"][0]["status"], "remove");
    assert!(
        !lab.root
            .path()
            .join(".gemini/config/skills/corral")
            .exists()
    );
}
