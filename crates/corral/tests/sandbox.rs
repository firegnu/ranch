//! The fence plugin boundary, using a synthetic launcher with a real child process.
mod common;
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    process::{Command, Output, Stdio},
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
            .env("FENCE_HOME", self.root.path().join("fence-home"))
            .env("SHELL", "/bin/sh")
            .env_remove("CODEX_SANDBOX")
            .env_remove("FENCE_RUN")
            .env_remove("CORRAL_NAME")
            .env_remove("CORRAL_INSTANCE")
            .env_remove("CORRAL_FENCE");
        c
    }
    fn out(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }
    fn caller(&self, parent: &Value) -> Command {
        let mut c = self.command();
        c.env("CORRAL_NAME", parent["name"].as_str().unwrap())
            .env("CORRAL_INSTANCE", parent["instance"].as_str().unwrap());
        c
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
        let out = self.out(&["ls"]);
        if let Ok(v) = serde_json::from_slice::<Value>(&out.stdout)
            && let Some(agents) = v["agents"].as_array()
        {
            for agent in agents {
                let _ = self.out(&["stop", agent["name"].as_str().unwrap(), "--timeout", "5"]);
            }
        }
    }
}

struct StoppedPen(i32);
impl StoppedPen {
    fn new(pid: i32) -> Self {
        assert_eq!(unsafe { libc::kill(pid, libc::SIGSTOP) }, 0);
        Self(pid)
    }
}
impl Drop for StoppedPen {
    fn drop(&mut self) {
        unsafe { libc::kill(self.0, libc::SIGCONT) };
    }
}

#[test]
fn fenced_kimi_hooks_record_the_main_agent_but_ignore_nested_agents() {
    let l = Lab::new();
    // macOS refuses the setuid /bin/ps in the real fence. Reproduce that without srt.
    let launch = if cfg!(target_os = "macos") {
        r#"/usr/bin/sandbox-exec -p '(version 1) (allow default) (deny process-exec (literal "/bin/ps"))' "$@""#
    } else {
        r#""$@""#
    };
    l.script(
        "fence",
        &format!(
            r#"
if [ "$1" = check ]; then echo '{{"ok":true}}'; exit; fi
while [ "$1" != -- ]; do
  if [ "$1" = --instance ]; then shift; export FENCE_RUN="$1"; fi
  shift
done
shift
{launch}
exit "$?"
"#
        ),
    );
    let kimi = l.script("kimi", r#"
printf '{"session_id":"main","cwd":"%s","source":"startup"}\n' "$CORRAL_CWD" | "$CORRAL_HELPER" __hook kimi:SessionStart > "$HOME/output" 2> "$HOME/errors"
printf '{"session_id":"main","cwd":"%s","prompt":[{"type":"text","text":"parent"}]}\n' "$CORRAL_CWD" | "$CORRAL_HELPER" __hook kimi:UserPromptSubmit >> "$HOME/output" 2>> "$HOME/errors"
/bin/sh -c 'printf "{\"session_id\":\"nested\",\"cwd\":\"%s\",\"source\":\"startup\"}\n" "$CORRAL_CWD" | "$CORRAL_HELPER" __hook kimi:SessionStart; sleep 0.1' >> "$HOME/output" 2>> "$HOME/errors"
touch "$HOME/finished"
exec /bin/cat
"#);
    l.json(&[
        "start",
        "test/kimi",
        "--sandbox",
        "dev",
        "--cwd",
        l.root.path().to_str().unwrap(),
        "--",
        &kimi,
    ]);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !l.root.path().join("finished").exists() {
        assert!(
            Instant::now() < deadline,
            "synthetic Kimi did not finish its hooks"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let status = l.json(&["status", "test/kimi"]);
    assert_eq!(status["state"], "working", "{status}");
    let events = fs::read_to_string(l.root.path().join("pens/test/kimi/events")).unwrap();
    assert!(events.contains("\"session_id\":\"main\""), "{events}");
    assert!(!events.contains("nested"), "{events}");
    assert!(fs::read(l.root.path().join("output")).unwrap().is_empty());
    assert!(fs::read(l.root.path().join("errors")).unwrap().is_empty());
}

#[test]
fn child_sandbox_does_not_wait_for_parent_pen_without_a_setting() {
    let l = Lab::new();
    let parent = l.json(&["start", "test/parent", "--", "/bin/cat"]);
    let status = l.json(&["status", "test/parent"]);
    let _stopped = StoppedPen::new(status["pen_pid"].as_i64().unwrap() as i32);
    let before = Instant::now();
    let out = l
        .caller(&parent)
        .args(["start", "test/child", "--", "/bin/cat"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(out.status.success(), "{v}");
    assert_eq!(v["sandbox"], Value::Null);
    assert!(
        before.elapsed() < Duration::from_secs(5),
        "start waited for the parent pen"
    );
}

#[test]
fn child_sandbox_keeps_setting_when_parent_pen_is_unresponsive() {
    let l = Lab::new();
    l.script(
        "fence",
        r#"
if [ "$1" = check ]; then echo '{"ok":true}'; exit; fi
while [ "$1" != -- ]; do shift; done
shift
exec "$@"
"#,
    );
    let parent = l.json(&["start", "test/parent", "--", "/bin/cat"]);
    l.json(&["child-sandbox", "test/parent", "dev"]);
    let status = l.json(&["status", "test/parent"]);
    let _stopped = StoppedPen::new(status["pen_pid"].as_i64().unwrap() as i32);
    let before = Instant::now();
    let out = l
        .caller(&parent)
        .args(["start", "test/child", "--", "/bin/cat"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(out.status.success(), "{v}");
    assert_eq!(v["sandbox"], json!({"profile":"dev"}));
    assert!(
        before.elapsed() < Duration::from_secs(5),
        "start waited for the parent pen"
    );
}

#[test]
fn child_sandbox_cannot_start_when_live_caller_identity_is_unreadable() {
    let l = Lab::new();
    l.script("fence", "echo '{\"ok\":true}'");
    let parent = l.json(&["start", "test/parent", "--", "/bin/cat"]);
    l.json(&["child-sandbox", "test/parent", "dev"]);
    let path = l.root.path().join("pens/test/parent/meta.json");
    let saved = fs::read(&path).unwrap();
    for content in ["{", "{}", r#"{"instance":""}"#] {
        fs::write(&path, content).unwrap();
        let out = l
            .caller(&parent)
            .args(["start", "test/child", "--", "/bin/cat"])
            .output()
            .unwrap();
        fs::write(&path, &saved).unwrap();
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(out.status.code(), Some(1), "{v}");
        assert_eq!(v["error"], "child_sandbox_unreadable");
        assert!(!l.root.path().join("pens/test/child/meta.json").exists());
    }
}

#[test]
fn child_sandbox_slow_check_cannot_overwrite_reopened_parent_setting() {
    let l = Lab::new();
    l.script(
        "fence",
        r#"
if [ "$3" = slow ]; then
    touch "$HOME/check-started"
    attempts=0
    while [ ! -f "$HOME/check-continue" ]; do
        attempts=$((attempts + 1))
        if [ "$attempts" -ge 100 ]; then echo '{"ok":false}'; exit 1; fi
        sleep 0.05
    done
fi
echo '{"ok":true}'
"#,
    );
    l.json(&["start", "test/parent", "--", "/bin/cat"]);
    let request = l
        .command()
        .args(["child-sandbox", "test/parent", "slow"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    eventually(|| l.root.path().join("check-started").exists());
    l.json(&["stop", "test/parent"]);
    l.json(&["start", "test/parent", "--", "/bin/cat"]);
    let saved = l.json(&["child-sandbox", "test/parent", "dev"]);
    fs::write(l.root.path().join("check-continue"), "").unwrap();
    let out = request.wait_with_output().unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(1), "{v}");
    assert_eq!(v["error"], "restarted");
    assert_eq!(l.json(&["child-sandbox", "test/parent"]), saved);
}

#[test]
fn child_sandbox_setting_is_instance_scoped_and_visible_without_wrapping_parent() {
    let l = Lab::new();
    l.script(
        "fence",
        r#"
printf '%s\n' "$@" > "$HOME/check-args"
echo '{"ok":true}'
"#,
    );
    let parent = l.json(&["start", "test/parent", "--", "/bin/cat"]);
    assert_eq!(
        l.json(&["child-sandbox", "test/parent"])["child_sandbox"],
        Value::Null
    );
    let set = l.json(&["child-sandbox", "test/parent", "dev"]);
    assert_eq!(
        set,
        json!({"ok":true,"name":"test/parent","instance":parent["instance"],"child_sandbox":{"profile":"dev"}})
    );
    assert_eq!(
        fs::read_to_string(l.root.path().join("check-args")).unwrap(),
        "check\n--profile\ndev\n"
    );
    assert_eq!(l.json(&["child-sandbox", "test/parent"]), set);
    let status = l.json(&["status", "test/parent"]);
    assert_eq!(status["child_sandbox"], json!({"profile":"dev"}));
    assert_eq!(status["sandbox"], Value::Null);
    assert_eq!(
        l.json(&["ls"])["agents"][0]["child_sandbox"],
        json!({"profile":"dev"})
    );
    assert_eq!(
        l.json(&["child-sandbox", "test/parent", "--clear"])["child_sandbox"],
        Value::Null
    );
    assert_eq!(
        l.json(&["status", "test/parent"])["child_sandbox"],
        Value::Null
    );
    l.json(&["child-sandbox", "test/parent", "dev"]);
    l.json(&["stop", "test/parent"]);
    assert_eq!(l.json(&["ls"])["agents"], json!([]));
    assert!(!l.root.path().join("pens/test/parent").exists());
    for args in [
        vec!["child-sandbox", "test/parent"],
        vec!["child-sandbox", "test/parent", "dev"],
        vec!["child-sandbox", "test/parent", "--clear"],
    ] {
        let out = l.out(&args);
        assert_eq!(out.status.code(), Some(2));
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["error"], "not_found");
    }
}

#[test]
fn child_sandbox_changes_apply_to_next_start_from_running_parent() {
    let l = Lab::new();
    l.script(
        "fence",
        r#"
if [ "$1" = check ]; then echo '{"ok":true}'; exit; fi
printf '%s\n' "$@" >> "$HOME/runs"
while [ "$1" != -- ]; do shift; done
shift
exec "$@"
"#,
    );
    let program = l.script(
        "parent",
        r#"
while IFS= read -r name; do
    "$HOME/corral" start "$name" -- /bin/cat > "$HOME/result.tmp"
    mv "$HOME/result.tmp" "$HOME/result"
done
"#,
    );
    l.json(&["start", "test/parent", "--", &program]);
    let start_child = |name: &str| {
        let result = l.root.path().join("result");
        let _ = fs::remove_file(&result);
        l.json(&["keys", "test/parent", &format!("text:{name}"), "enter"]);
        eventually(|| result.exists());
        let v: Value = serde_json::from_slice(&fs::read(result).unwrap()).unwrap();
        assert_eq!(v["ok"], true, "{v}");
        v
    };
    assert_eq!(start_child("test/before")["sandbox"], Value::Null);
    l.json(&["child-sandbox", "test/parent", "dev"]);
    assert_eq!(
        start_child("test/first")["sandbox"],
        json!({"profile":"dev"})
    );
    l.json(&["child-sandbox", "test/parent", "other"]);
    assert_eq!(
        start_child("test/second")["sandbox"],
        json!({"profile":"other"})
    );
    l.json(&["child-sandbox", "test/parent", "--clear"]);
    assert_eq!(start_child("test/after")["sandbox"], Value::Null);
    for (name, profile) in [("test/first", "dev"), ("test/second", "other")] {
        assert_eq!(
            l.json(&["status", name])["sandbox"],
            json!({"profile":profile})
        );
    }
    assert_eq!(l.json(&["status", "test/before"])["sandbox"], Value::Null);
    assert_eq!(l.json(&["status", "test/parent"])["sandbox"], Value::Null);
    eventually(|| {
        fs::read_to_string(l.root.path().join("runs")).is_ok_and(|s| s.contains("test/second\n"))
    });
}

#[test]
fn child_sandbox_invalid_or_unreadable_record_blocks_start() {
    let l = Lab::new();
    let parent = l.json(&["start", "test/parent", "--", "/bin/cat"]);
    let path = l.root.path().join("pens/test/parent/child-sandbox.json");
    for content in [
        "{".to_owned(),
        json!({"instance":parent["instance"],"child_sandbox":{"profile":42}}).to_string(),
    ] {
        fs::write(&path, content).unwrap();
        let out = l
            .caller(&parent)
            .args(["start", "test/child", "--", "/bin/cat"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1));
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["error"], "child_sandbox_unreadable", "{v}");
        assert!(!l.root.path().join("pens/test/child/meta.json").exists());
    }
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    let out = l
        .caller(&parent)
        .args(["start", "test/child", "--", "/bin/cat"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["error"], "child_sandbox_unreadable");
    assert!(!l.root.path().join("pens/test/child/meta.json").exists());
    fs::remove_dir(&path).unwrap();
}

#[test]
fn child_sandbox_explicit_options_override_setting_and_bad_record() {
    let l = Lab::new();
    l.script(
        "fence",
        r#"
if [ "$1" = check ]; then echo '{"ok":true}'; exit; fi
while [ "$1" != -- ]; do shift; done
shift
exec "$@"
"#,
    );
    let parent = l.json(&["start", "test/parent", "--", "/bin/cat"]);
    l.json(&["child-sandbox", "test/parent", "dev"]);
    let path = l.root.path().join("pens/test/parent/child-sandbox.json");
    for (prefix, corrupt) in [("configured", false), ("corrupt", true)] {
        if corrupt {
            fs::write(&path, "{").unwrap();
        }
        for (suffix, flags, expected) in [
            (
                "explicit",
                vec!["--sandbox", "other"],
                json!({"profile":"other"}),
            ),
            ("off", vec!["--no-sandbox"], Value::Null),
        ] {
            let out = l
                .caller(&parent)
                .args(["start", &format!("test/{prefix}-{suffix}")])
                .args(flags)
                .args(["--", "/bin/cat"])
                .output()
                .unwrap();
            let v: Value = serde_json::from_slice(&out.stdout).unwrap();
            assert!(out.status.success(), "{v}");
            assert_eq!(v["sandbox"], expected);
        }
    }
    let out = l
        .caller(&parent)
        .args([
            "start",
            "test/conflict",
            "--sandbox",
            "dev",
            "--no-sandbox",
            "--",
            "/bin/cat",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["error"], "usage");
    assert!(!l.root.path().join("pens/test/conflict/meta.json").exists());
}

#[test]
fn child_sandbox_absent_stale_or_stopped_caller_keeps_default() {
    let l = Lab::new();
    l.script("fence", "echo '{\"ok\":true}'");
    let parent = l.json(&["start", "test/parent", "--", "/bin/cat"]);
    let path = l.root.path().join("pens/test/parent/child-sandbox.json");
    let start_plain = |parent: &Value, name: &str| {
        let out = l
            .caller(parent)
            .args(["start", name, "--", "/bin/cat"])
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(out.status.success(), "{v}");
        assert_eq!(v["sandbox"], Value::Null);
    };
    start_plain(&parent, "test/unset");
    l.json(&["child-sandbox", "test/parent", "dev"]);
    let old_record = fs::read(&path).unwrap();
    fs::remove_file(&path).unwrap();
    start_plain(&parent, "test/missing");
    l.json(&["stop", "test/parent"]);
    let reopened = l.json(&["start", "test/parent", "--", "/bin/cat"]);
    assert_ne!(parent["instance"], reopened["instance"]);
    // A previous CLI may leave a file from the old run behind.
    fs::write(&path, old_record).unwrap();
    start_plain(&reopened, "test/stale-record");
    for v in [
        l.json(&["child-sandbox", "test/parent"]),
        l.json(&["status", "test/parent"]),
    ] {
        assert_eq!(v.get("child_sandbox"), Some(&Value::Null));
    }
    let list = l.json(&["ls"]);
    let row = list["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "test/parent")
        .unwrap();
    assert_eq!(row.get("child_sandbox"), Some(&Value::Null));
    l.json(&["child-sandbox", "test/parent", "dev"]);
    start_plain(&parent, "test/stale-caller");
    // The new instance is stopped; even a broken policy must not affect its old caller.
    l.json(&["stop", "test/parent"]);
    fs::write(&path, "{").unwrap();
    start_plain(&reopened, "test/stopped-caller");
}

#[test]
fn child_sandbox_failed_check_never_saves_and_start_rechecks() {
    let l = Lab::new();
    let parent = l.json(&["start", "test/parent", "--", "/bin/cat"]);
    let path = l.root.path().join("pens/test/parent/child-sandbox.json");
    let check_error = |expected: &str| {
        let out = l.out(&["child-sandbox", "test/parent", "other"]);
        assert_eq!(out.status.code(), Some(1));
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["error"], expected);
    };
    check_error("sandbox_unavailable");
    assert!(!path.exists());
    l.script(
        "fence",
        "echo '{\"ok\":false,\"error\":\"unknown_profile\"}'; exit 1",
    );
    check_error("sandbox_failed");
    assert!(!path.exists());
    l.script("fence", "echo '{\"ok\":true}'");
    l.json(&["child-sandbox", "test/parent", "dev"]);
    let saved = fs::read(&path).unwrap();
    l.script(
        "fence",
        "echo '{\"ok\":false,\"error\":\"unknown_profile\"}'; exit 1",
    );
    check_error("sandbox_failed");
    assert_eq!(fs::read(&path).unwrap(), saved);
    for expected in ["sandbox_failed", "sandbox_unavailable"] {
        if expected == "sandbox_unavailable" {
            fs::remove_file(l.root.path().join("fence")).unwrap();
        }
        let out = l
            .caller(&parent)
            .args(["start", "test/child", "--", "/bin/cat"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1));
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["error"], expected);
        assert!(!l.root.path().join("pens/test/child/meta.json").exists());
    }
    assert_eq!(
        l.json(&["child-sandbox", "test/parent"])["child_sandbox"],
        json!({"profile":"dev"})
    );
    assert_eq!(
        l.json(&["child-sandbox", "test/parent", "--clear"])["child_sandbox"],
        Value::Null
    );
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
