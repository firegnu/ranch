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
