//! The fence plugin boundary, using a synthetic launcher with a real child process.
mod common;
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    process::{Command, Output},
    time::{Duration, Instant},
};
struct Lab {
    root: tempfile::TempDir,
}
impl Lab {
    fn new() -> Self {
        let root = tempfile::tempdir_in("/tmp").unwrap();
        common::copy_exe(env!("CARGO_BIN_EXE_corral"), root.path().join("corral"));
        Self { root }
    }
    fn command(&self) -> Command {
        let mut c = Command::new(self.root.path().join("corral"));
        c.env("HOME", self.root.path())
            .env("CORRAL_HOME", self.root.path().join("pens"))
            .env("SHELL", "/bin/sh")
            .env_remove("CODEX_SANDBOX")
            .env_remove("FENCE_RUN")
            .env_remove("CORRAL_FENCE");
        c
    }
    fn out(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }
    fn json(&self, args: &[&str]) -> Value {
        let out = self.out(args);
        assert!(
            out.status.success(),
            "{args:?}: {} {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
    fn script(&self, name: &str, body: &str) -> String {
        let path = self.root.path().join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path.to_str().unwrap().into()
    }
}
impl Drop for Lab {
    fn drop(&mut self) {
        let _ = self.out(&["stop", "test/fence", "--timeout", "5"]);
    }
}
fn eventually(mut f: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(5);
    while !f() {
        assert!(Instant::now() < end, "condition not reached");
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[test]
fn sandbox_check_failure_does_not_start_pen_and_preserves_fence_error() {
    let l = Lab::new();
    let args = ["start", "test/fence", "--sandbox", "dev", "--", "/bin/cat"];
    let out = l.out(&args);
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["error"], "sandbox_unavailable");
    l.script("fence", "printf '%s\\n' '{\"ok\":false,\"error\":\"unsupported_kind\",\"supported\":[\"claude\"]}'; exit 1");
    let out = l.out(&args);
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["error"], "sandbox_failed");
    assert_eq!(v["fence"]["error"], "unsupported_kind");
    assert!(!l.root.path().join("pens/test/fence/meta.json").exists());
    assert_eq!(l.json(&["ls"])["agents"], json!([]));
}

#[test]
fn wrapped_agent_keeps_kind_hooks_prompt_and_lifecycle() {
    let l = Lab::new();
    l.script("fence", r#"
printf '%s\n' "$@" >> "$HOME/fence-args"
if [ "$1" = check ]; then printf '%s\n' "$FENCE_HOME" > "$HOME/check-env"; echo '{"ok":true}'; exit; fi
while [ "$1" != -- ]; do shift; done
shift
trap ':' HUP INT TERM
"$@" <&0 &
child=$!
echo "$child" > "$HOME/child-pid"
while kill -0 "$child" 2>/dev/null; do wait "$child"; result=$?; done
exit "$result"
"#);
    let program = l.script(
        "claude",
        r#"
printf '%s\n' "$@" > "$HOME/agent-args"
printf '%s\n' "$CORRAL_NAME" > "$HOME/agent-env"
trap 'exit 7' HUP
while :; do sleep 0.05; done
"#,
    );
    let fence_home = format!("FENCE_HOME={}/isolated-fence", l.root.path().display());
    let started = l.json(&[
        "start",
        "test/fence",
        "--cwd",
        "/tmp",
        "--sandbox",
        "dev",
        "--prompt",
        "first prompt",
        "--env",
        &fence_home,
        "--",
        &program,
        "user-argument",
    ]);
    assert_eq!(started["kind"], "claude");
    assert_eq!(started["sandbox"], json!({"profile":"dev"}));
    eventually(|| l.root.path().join("agent-args").exists());
    let args = fs::read_to_string(l.root.path().join("agent-args")).unwrap();
    assert!(args.contains("--settings\n"), "{args}");
    assert!(args.contains("first prompt"));
    assert!(args.contains("user-argument"));
    assert_eq!(
        fs::read_to_string(l.root.path().join("agent-env"))
            .unwrap()
            .trim(),
        "test/fence"
    );
    assert!(
        fs::read_to_string(l.root.path().join("check-env"))
            .unwrap()
            .contains("isolated-fence")
    );
    let meta: Value =
        serde_json::from_slice(&fs::read(l.root.path().join("pens/test/fence/meta.json")).unwrap())
            .unwrap();
    assert_eq!(meta["argv"][0], program);
    assert!(meta["sandbox"]["fence"].as_str().unwrap().starts_with('/'));
    for args in [&["status", "test/fence"][..], &["where", "test/fence"]] {
        assert_eq!(l.json(args)["sandbox"], json!({"profile":"dev"}));
    }
    assert_eq!(
        l.json(&["ls"])["agents"][0]["sandbox"],
        json!({"profile":"dev"})
    );
    let child = fs::read_to_string(l.root.path().join("child-pid")).unwrap();
    let state = || {
        let out = Command::new("ps")
            .args(["-o", "stat=", "-p", child.trim()])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };
    l.json(&["pause", "test/fence"]);
    eventually(|| state().starts_with('T'));
    l.json(&["resume", "test/fence"]);
    eventually(|| !state().starts_with('T'));
    let result = l.json(&["stop", "test/fence", "--timeout", "10"]);
    assert_eq!(result["exit_code"], 7, "{result}");
    assert_eq!(result["stopped_by"], "SIGHUP", "{result}");
    assert_eq!(l.json(&["ls"])["agents"], json!([]));
}

#[test]
fn fence_guard_allows_hooks_and_guide_but_rejects_user_commands() {
    let l = Lab::new();
    let out = l
        .command()
        .env("FENCE_RUN", "instance")
        .arg("ls")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(6));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["error"], "sandbox");
    for args in [&["guide"][..], &["__hook", "Stop"]] {
        assert!(
            l.command()
                .env("FENCE_RUN", "instance")
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
}

#[test]
fn old_metadata_returns_null_sandbox() {
    let l = Lab::new();
    let v = l.json(&["start", "test/fence", "--", "/bin/cat"]);
    assert_eq!(v.get("sandbox"), Some(&Value::Null));
    let path = l.root.path().join("pens/test/fence/meta.json");
    let mut meta: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    meta.as_object_mut().unwrap().remove("sandbox");
    fs::write(path, meta.to_string()).unwrap();
    assert_eq!(
        l.json(&["status", "test/fence"]).get("sandbox"),
        Some(&Value::Null)
    );
    assert_eq!(
        l.json(&["ls"])["agents"][0].get("sandbox"),
        Some(&Value::Null)
    );
}

#[test]
fn losing_runner_kills_its_remaining_process_group() {
    let l = Lab::new();
    let fence = l.script(
        "external-fence",
        r#"
if [ "$1" = check ]; then echo '{"ok":true}'; exit; fi
trap '' HUP
/bin/sleep 60 &
echo "$!" > "$HOME/leftover"
wait
"#,
    );
    let out = l
        .command()
        .env("CORRAL_FENCE", &fence)
        .args(["start", "test/fence", "--sandbox", "dev", "--", "/bin/cat"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], true, "{v}");
    eventually(|| l.root.path().join("leftover").exists());
    let child: i32 = fs::read_to_string(l.root.path().join("leftover"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let status = l.json(&["status", "test/fence"]);
    unsafe {
        libc::kill(status["agent_pid"].as_i64().unwrap() as i32, libc::SIGKILL);
    }
    eventually(|| {
        let out = Command::new("ps")
            .args(["-o", "stat=", "-p", &child.to_string()])
            .output()
            .unwrap();
        let state = String::from_utf8_lossy(&out.stdout);
        state.trim().is_empty() || state.trim().starts_with('Z')
    });
    eventually(|| l.json(&["ls"])["agents"] == json!([]));
}
