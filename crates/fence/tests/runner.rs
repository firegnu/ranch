//! Exercise the unchanged runner with a synthetic srt event source, on either host OS.
use serde_json::{Value, json};
use std::{fs, process::Command};

fn record(lines: &[&str]) -> (Vec<Value>, Value) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let module = root.join("lib/srt/node_modules/@anthropic-ai/sandbox-runtime");
    fs::create_dir_all(module.join("dist")).unwrap();
    fs::create_dir_all(root.join("share/fence")).unwrap();
    fs::write(module.join("package.json"), r#"{"type":"module"}"#).unwrap();
    fs::write(module.join("dist/index.js"), format!(r#"
let callback;
let events = [];
export const SandboxRuntimeConfigSchema = {{ parse: value => value }};
export const SandboxManager = {{
  getSandboxViolationStore: () => ({{
    subscribe: fn => {{ callback = fn; }}, getTotalCount: () => events.length
  }}),
  initialize: async () => {{
    events = {}.map(line => ({{ line, timestamp: new Date(1000), command: 'private prompt', encodedCommand: 'private' }}));
    callback(events);
  }},
  wrapWithSandbox: async command => "/bin/sh -c '" + command.replaceAll("'", "'\\''") + "'",
  reset: async () => {{}}
}};
"#, json!(lines))).unwrap();
    fs::write(root.join("settings.json"), "{}").unwrap();
    let runner = root.join("share/fence/runner.mjs");
    fs::copy(
        concat!(env!("CARGO_MANIFEST_DIR"), "/resources/runner.mjs"),
        &runner,
    )
    .unwrap();
    let out = Command::new("node")
        .arg(runner)
        .env("HOME", root)
        .env("FENCE_HOME", root.join("fence"))
        .arg("--settings")
        .arg(root.join("settings.json"))
        .arg("--denials")
        .arg(root.join("denials.jsonl"))
        .arg("--exit")
        .arg(root.join("exit.json"))
        .args(["--", "/bin/sh", "-c", "exit 0"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let events = fs::read_to_string(root.join("denials.jsonl")).unwrap();
    assert!(!events.contains("private"));
    let events: Vec<Value> = events
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(events.len(), lines.len());
    let monitor = serde_json::from_slice(&fs::read(root.join("monitor.json")).unwrap()).unwrap();
    (events, monitor)
}

#[test]
fn macos_file_and_network_events_keep_the_public_categories() {
    let (events, monitor) = record(&[
        "Sandbox: sh(123) deny(1) file-read-data /synthetic/key",
        "Sandbox: sh(123) deny(1) file-write-create /synthetic/file with spaces",
        "deny network-outbound example.com:443",
        "unclassified diagnostic",
    ]);
    for (event, (kind, target)) in events.iter().zip([
        ("file-read", json!("/synthetic/key")),
        ("file-write", json!("/synthetic/file with spaces")),
        ("network", json!("example.com:443")),
        ("other", Value::Null),
    ]) {
        assert_eq!(event["type"], kind);
        assert_eq!(event["target"], target);
    }
    assert_eq!(monitor["monitors"]["files"], true);
}

#[test]
fn linux_write_attempt_events_confirm_file_monitoring() {
    let (events, monitor) = record(&[
        "deny openat /synthetic/protected",
        "deny renameat2 /synthetic/file with spaces",
        "deny syscall /synthetic/fallback",
        "deny openat /synthetic/file-read denied",
        "deny openat /synthetic/deny network-outbound example.com",
    ]);
    for (event, target) in events.iter().zip([
        "/synthetic/protected",
        "/synthetic/file with spaces",
        "/synthetic/fallback",
        "/synthetic/file-read denied",
        "/synthetic/deny network-outbound example.com",
    ]) {
        assert_eq!(event["type"], "file-write", "{event}");
        assert_eq!(event["target"], target);
    }
    assert_eq!(monitor["monitors"]["files"], true);
}
