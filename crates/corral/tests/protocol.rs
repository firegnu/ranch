// From Saddle `crates/corral-core/tests/protocol.rs` at commit `a31dea2`; ranch has changed it since.
//! Public CLI and v1 pen transport, using only isolated synthetic agents.
mod common;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::PathBuf,
    process::{Command, Output},
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
        let mut c = Command::new(&self.core);
        c.env("HOME", self.root.path())
            .env("CORRAL_HOME", self.root.path().join("pens"))
            .env("SHELL", "/bin/sh")
            .env_remove("CODEX_SANDBOX");
        c
    }
    fn out(&self, args: &[&str]) -> Output {
        self.cmd().args(args).output().unwrap()
    }
    fn json(&self, args: &[&str]) -> Value {
        let o = self.out(args);
        assert!(
            o.status.success(),
            "{args:?} {}",
            String::from_utf8_lossy(&o.stdout)
        );
        serde_json::from_slice(&o.stdout).unwrap()
    }
    fn attach(&self, rows: u16, cols: u16) -> (UnixStream, Value) {
        let mut s = UnixStream::connect(self.root.path().join("pens/lab/main/sock")).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        writeln!(
            s,
            "{}",
            json!({"proto":1,"op":"attach","rows":rows,"cols":cols})
        )
        .unwrap();
        let mut line = vec![];
        loop {
            let mut b = [0];
            s.read_exact(&mut b).unwrap();
            line.push(b[0]);
            if b[0] == b'\n' {
                break;
            }
        }
        let hello = serde_json::from_slice(&line).unwrap();
        (s, hello)
    }
    fn event(&self, ev: &str, body: Value) {
        let mut child = self
            .cmd()
            .args(["__hook", ev])
            .env(
                "CORRAL_EVENTS",
                self.root.path().join("pens/lab/main/events"),
            )
            .env(
                "CORRAL_INSTANCE",
                self.json(&["status", "lab/main"])["instance"]
                    .as_str()
                    .unwrap(),
            )
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        serde_json::to_writer(child.stdin.take().unwrap(), &body).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        assert!(out.stdout.is_empty());
    }
    fn recognized(&self) {
        let path = self.root.path().join("claude");
        fs::write(&path, "#!/bin/sh\nexec /bin/cat\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        self.json(&[
            "start",
            "lab/main",
            "--cwd",
            "/tmp",
            "--",
            path.to_str().unwrap(),
        ]);
    }
    fn color_agent(&self, mode: &str) -> Value {
        let start = self.json(&[
            "start",
            "lab/main",
            "--cwd",
            self.root.path().to_str().unwrap(),
            "--env",
            &format!("R50B_DIR={}", self.root.path().display()),
            "--env",
            &format!("R50B_MODE={mode}"),
            "--",
            std::env::current_exe().unwrap().to_str().unwrap(),
            "--exact",
            "terminal_colors_synthetic_agent",
            "--ignored",
            "--nocapture",
        ]);
        eventually(|| self.root.path().join("ready").exists());
        start
    }
    fn color_result(&self) -> Value {
        let path = self.root.path().join("result");
        // Allow the partial-input child its full read deadline plus result serialization.
        let deadline = Instant::now() + Duration::from_secs(12);
        while !path.exists() {
            assert!(
                Instant::now() < deadline,
                "color agent did not finish reading"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }
    fn raw_output(&self) -> Vec<u8> {
        use base64::{Engine, engine::general_purpose::STANDARD as B64};
        let mut socket = UnixStream::connect(self.root.path().join("pens/lab/main/sock")).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        writeln!(socket, "{}", json!({"op":"read","bytes":16000})).unwrap();
        let mut reply = Vec::new();
        socket.read_to_end(&mut reply).unwrap();
        let reply: Value = serde_json::from_slice(&reply).unwrap();
        B64.decode(reply["data"].as_str().unwrap()).unwrap()
    }
}
impl Drop for Lab {
    fn drop(&mut self) {
        for name in ["lab/main", "lab/main-1", "lab/main-2", "lab/main-3"] {
            let _ = self.out(&["stop", name, "--timeout", "4"]);
        }
    }
}
fn input(s: &mut UnixStream, kind: u8, bytes: &[u8]) {
    s.write_all(&[kind]).unwrap();
    s.write_all(&(bytes.len() as u32).to_be_bytes()).unwrap();
    s.write_all(bytes).unwrap();
}
fn eventually(mut check: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(3);
    while !check() {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[test]
fn terminal_colors_cli_sets_reads_clears_and_rejects_invalid_arguments() {
    let lab = Lab::new();
    let unset = json!({"ok":true,"terminal_colors":null});
    assert_eq!(lab.json(&["terminal-colors"]), unset);
    let colors = json!({"ok":true,"terminal_colors":{"fg":"#4c4f69","bg":"#eff1f5"}});
    assert_eq!(
        lab.json(&["terminal-colors", "--fg", "#4C4F69", "--bg", "#EFF1F5"]),
        colors
    );
    assert_eq!(lab.json(&["terminal-colors"]), colors);
    for args in [
        vec!["--fg", "#123456"],
        vec!["--bg", "#123456"],
        vec!["--fg", "#123", "--bg", "#abcdef"],
        vec!["--fg", "123456", "--bg", "#abcdef"],
        vec!["--fg", "#123456", "--bg", "#abcdeg"],
        vec!["--fg", "#123456", "--bg", "#abcdef", "--clear"],
        vec!["--fg"],
        vec!["--clear", "extra"],
        vec!["--unknown"],
    ] {
        let mut command = vec!["terminal-colors"];
        command.extend(args);
        let out = lab.out(&command);
        assert_eq!(out.status.code(), Some(1), "{command:?}");
        let value: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(value["error"], "usage", "{command:?}: {value}");
        assert_eq!(lab.json(&["terminal-colors"]), colors);
    }
    assert_eq!(lab.json(&["terminal-colors", "--clear"]), unset);
    assert_eq!(lab.json(&["terminal-colors"]), unset);
    assert_eq!(lab.json(&["terminal-colors", "--clear"]), unset);
    let help = lab.out(&["terminal-colors", "--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--fg COLOR --bg COLOR | --clear"));
}

#[test]
fn terminal_colors_creates_private_runtime_directories() {
    use std::os::unix::process::CommandExt;
    let lab = Lab::new();
    let mut cmd = lab.cmd();
    unsafe {
        cmd.pre_exec(|| {
            libc::umask(0o022);
            Ok(())
        });
    }
    let out = cmd
        .args(["terminal-colors", "--fg", "#4c4f69", "--bg", "#eff1f5"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    for dir in ["pens", "pens/.runtime"] {
        assert_eq!(
            fs::metadata(lab.root.path().join(dir))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700,
            "{dir}"
        );
    }
}

#[test]
fn terminal_colors_read_reports_invalid_settings_without_changing_the_file() {
    let lab = Lab::new();
    let path = lab.root.path().join("pens/.runtime/terminal-colors.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    for contents in [
        r##"{"fg":"red","bg":"#000000"}"##,
        "{broken",
        r##"{"fg":"#ffffff"}"##,
    ] {
        fs::write(&path, contents).unwrap();
        let out = lab.out(&["terminal-colors"]);
        assert_eq!(out.status.code(), Some(1));
        let reply: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(reply["ok"], false);
        assert_eq!(reply["error"], "invalid_settings", "{reply}");
        assert!(
            reply["message"]
                .as_str()
                .unwrap()
                .contains("terminal-colors.json"),
            "{reply}"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), contents);
    }
}

const COLOR_QUERY: &[u8] = b"\x1b]10;?\x07\x1b]11;?\x1b\\";
const COLOR_REPLY: &[u8] = b"\x1b]10;rgb:4c4c/4f4f/6969\x07\x1b]11;rgb:efef/f1f1/f5f5\x1b\\";
const COLOR_PARTIAL_INPUT_BYTES: usize = 2 * 1024 * 1024;

#[test]
#[ignore = "synthetic PTY child for terminal color tests"]
fn terminal_colors_synthetic_agent() {
    let Some(dir) = std::env::var_os("R50B_DIR").map(PathBuf::from) else {
        return;
    };
    let mode = std::env::var("R50B_MODE").unwrap();
    unsafe {
        let mut term = std::mem::zeroed();
        assert_eq!(libc::tcgetattr(0, &mut term), 0);
        libc::cfmakeraw(&mut term);
        assert_eq!(libc::tcsetattr(0, libc::TCSANOW, &term), 0);
    }
    if mode == "upgrade" {
        std::io::stdout().write_all(b"\x1b]11;?").unwrap();
        std::io::stdout().flush().unwrap();
    }
    fs::write(dir.join("ready"), b"").unwrap();
    eventually(|| dir.join("go").exists());
    let mut received = Vec::new();
    if matches!(mode.as_str(), "delayed" | "partial") {
        let mut first = [0];
        std::io::stdin().read_exact(&mut first).unwrap();
        received.extend(first);
    }
    let begin = Instant::now();
    std::io::stdout()
        .write_all(if mode == "upgrade" {
            b"\x1b\\"
        } else {
            COLOR_QUERY
        })
        .unwrap();
    std::io::stdout().flush().unwrap();
    let mut deadline = begin + Duration::from_millis(if mode == "partial" { 10000 } else { 400 });
    let mut received_all = false;
    let mut last_read_ms = None;
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        let mut fd = libc::pollfd {
            fd: 0,
            events: libc::POLLIN,
            revents: 0,
        };
        if unsafe { libc::poll(&mut fd, 1, remaining.as_millis() as i32) } > 0 {
            let mut bytes = [0; 65536];
            let n = std::io::stdin().read(&mut bytes).unwrap();
            if n == 0 {
                break;
            }
            received.extend_from_slice(&bytes[..n]);
            last_read_ms = Some(begin.elapsed().as_millis());
            if mode == "partial"
                && !received_all
                && received.len() >= COLOR_PARTIAL_INPUT_BYTES + COLOR_REPLY.len()
            {
                // Keep observing after the expected bytes to catch duplicates or extra input.
                deadline = deadline.min(Instant::now() + Duration::from_millis(200));
                received_all = true;
            }
        }
    }
    fs::write(
        dir.join("result.tmp"),
        serde_json::to_vec(&json!({"bytes":received,"last_read_ms":last_read_ms})).unwrap(),
    )
    .unwrap();
    fs::rename(dir.join("result.tmp"), dir.join("result")).unwrap();
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}

#[test]
fn terminal_colors_reply_promptly_only_without_attached_clients() {
    for (configured, attached) in [(true, false), (false, false), (true, true)] {
        let lab = Lab::new();
        if configured {
            lab.json(&["terminal-colors", "--fg", "#4c4f69", "--bg", "#eff1f5"]);
        }
        lab.color_agent("normal");
        let client = attached.then(|| lab.attach(40, 120).0);
        fs::write(lab.root.path().join("go"), b"").unwrap();
        let result = lab.color_result();
        assert_eq!(
            result["bytes"],
            json!(if configured && !attached {
                COLOR_REPLY
            } else {
                b""
            })
        );
        if configured && !attached {
            assert!(result["last_read_ms"].as_u64().unwrap() < 250, "{result}");
        }
        if let Some(mut client) = client {
            let mut bytes = vec![0; COLOR_QUERY.len()];
            client.read_exact(&mut bytes).unwrap();
            assert_eq!(bytes, COLOR_QUERY);
        }
        let status = lab.json(&["status", "lab/main"]);
        assert!(status["last_human_input"].is_null());
        assert_eq!(status["recent_sends"], json!([]));
    }
}

#[test]
fn terminal_colors_start_warns_on_bad_settings_without_failing() {
    let lab = Lab::new();
    let path = lab.root.path().join("pens/.runtime/terminal-colors.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, b"{broken").unwrap();
    let start = lab.color_agent("normal");
    assert!(
        start["warnings"].as_array().is_some_and(|warnings| warnings
            .iter()
            .any(|v| v.as_str().unwrap().contains("terminal-colors"))),
        "{start}"
    );
    fs::write(lab.root.path().join("go"), b"").unwrap();
    assert_eq!(lab.color_result()["bytes"], json!([]));
}

#[test]
fn terminal_colors_reply_bypasses_send_delay_but_never_splits_a_chunk() {
    use base64::{Engine, engine::general_purpose::STANDARD as B64};
    for (mode, size) in [("delayed", 1), ("partial", COLOR_PARTIAL_INPUT_BYTES)] {
        let lab = Lab::new();
        lab.json(&["terminal-colors", "--fg", "#4c4f69", "--bg", "#eff1f5"]);
        lab.color_agent(mode);
        fs::write(lab.root.path().join("go"), b"").unwrap();
        let mut sending = UnixStream::connect(lab.root.path().join("pens/lab/main/sock")).unwrap();
        let mut expected = vec![b'x'; size];
        writeln!(sending, "{}", json!({"op":"send","request_id":"colors-queued","chunks":[{"data":B64.encode(&expected),"delay":3.0},{"data":B64.encode(b"later")}]})).unwrap();
        expected.extend_from_slice(COLOR_REPLY);
        let result = lab.color_result();
        let bytes: Vec<u8> = serde_json::from_value(result["bytes"].clone()).unwrap();
        assert_eq!(
            bytes.len(),
            expected.len(),
            "{mode}: missing or extra bytes"
        );
        assert!(bytes == expected, "{mode}: reply split the input chunk");
        if mode == "delayed" {
            assert!(result["last_read_ms"].as_u64().unwrap() < 250, "{result}");
        }
        let status = lab.json(&["status", "lab/main"]);
        assert!(status["last_human_input"].is_null());
        assert_eq!(status["recent_sends"].as_array().unwrap().len(), 1);
        assert_eq!(status["recent_sends"][0]["request_id"], "colors-queued");
    }
}

#[test]
fn terminal_colors_upgrade_keeps_start_colors_and_partial_query() {
    let lab = Lab::new();
    lab.json(&["terminal-colors", "--fg", "#4c4f69", "--bg", "#eff1f5"]);
    lab.color_agent("upgrade");
    eventually(|| lab.raw_output().ends_with(b"\x1b]11;?"));
    lab.json(&["terminal-colors", "--clear"]);
    let target = lab.root.path().join("new-corral");
    common::copy_exe(&lab.core, &target);
    let upgraded = lab.json(&["upgrade", "lab/main", "--exe", target.to_str().unwrap()]);
    assert_eq!(upgraded["result"], "complete", "{upgraded}");
    fs::write(lab.root.path().join("go"), b"").unwrap();
    assert_eq!(
        lab.color_result()["bytes"],
        json!(b"\x1b]11;rgb:efef/f1f1/f5f5\x1b\\".as_slice())
    );
}
#[test]
fn attachments_share_one_writer_and_transfer_ownership_without_restarting() {
    let lab = Lab::new();
    let start = lab.json(&["start", "lab/main", "--", "/bin/cat"]);
    let (mut first, a) = lab.attach(24, 80);
    let (mut second, b) = lab.attach(30, 90);
    assert_eq!(a["readonly"], false);
    assert_eq!(b["readonly"], true);
    input(&mut second, b'i', b"ignored-input\r");
    std::thread::sleep(Duration::from_millis(50));
    assert!(
        !String::from_utf8_lossy(&lab.out(&["read", "lab/main"]).stdout).contains("ignored-input")
    );
    input(&mut first, b'i', b"writer-one\r");
    eventually(|| {
        String::from_utf8_lossy(&lab.out(&["read", "lab/main"]).stdout).contains("writer-one")
    });
    assert_eq!(
        lab.out(&["send", "lab/main", "should-not-arrive"])
            .status
            .code(),
        Some(8)
    );
    drop(first);
    eventually(|| lab.json(&["status", "lab/main"])["attached"] == 1);
    input(
        &mut second,
        b'r',
        &[40u16.to_be_bytes(), 100u16.to_be_bytes()].concat(),
    );
    input(&mut second, b'i', b"writer-two\r");
    eventually(|| {
        String::from_utf8_lossy(&lab.out(&["read", "lab/main"]).stdout).contains("writer-two")
    });
    assert_eq!(
        lab.json(&["status", "lab/main"])["instance"],
        start["instance"]
    );
    lab.json(&["send", "lab/main", "forced", "--force"]);
}
#[test]
fn events_filter_subagents_keep_full_reply_and_do_not_resend_unconfirmed_input() {
    let lab = Lab::new();
    lab.recognized();
    assert_eq!(lab.json(&["status", "lab/main"])["state"], "starting");
    assert_eq!(
        lab.out(&["send", "lab/main", "not-yet"]).status.code(),
        Some(7)
    );
    lab.event(
        "UserPromptSubmit",
        json!({"session_id":"main","prompt":"first"}),
    );
    lab.event(
        "SessionStart",
        json!({"session_id":"main","cwd":"/tmp","transcript_path":"t"}),
    );
    assert_eq!(lab.json(&["status", "lab/main"])["state"], "working");
    lab.event("SessionStart", json!({"session_id":"sub","cwd":"/tmp"}));
    lab.event(
        "Stop",
        json!({"session_id":"sub","last_assistant_message":"not main"}),
    );
    assert_eq!(lab.json(&["status", "lab/main"])["state"], "working");
    lab.event(
        "PermissionRequest",
        json!({"session_id":"main","tool_name":"shell"}),
    );
    assert_eq!(
        lab.json(&["wait", "lab/main", "--timeout", "2"])["result"],
        "blocked"
    );
    let reply = "完整\r\n```rust\nfn main() {}\n```\u{0}";
    lab.event(
        "Stop",
        json!({"session_id":"main","last_assistant_message":reply,"background_tasks":[]}),
    );
    assert_eq!(lab.json(&["reply", "lab/main"])["text"], reply);
    let sent = lab.out(&["send", "lab/main", "one delivery", "--timeout", "0.1"]);
    assert_eq!(sent.status.code(), Some(3));
    // send returning does not mean cat's echo has reached the pen yet.
    eventually(|| {
        let out = lab.out(&["read", "lab/main"]);
        assert!(out.status.success());
        String::from_utf8_lossy(&out.stdout)
            .matches("one delivery\n")
            .count()
            >= 2
    });
    // Queue a barrier after send: both echoes mean cat consumed all preceding input.
    lab.json(&["keys", "lab/main", "text:echo-barrier", "enter"]);
    let mut output = String::new();
    eventually(|| {
        let out = lab.out(&["read", "lab/main"]);
        assert!(out.status.success());
        output = String::from_utf8(out.stdout).unwrap();
        output.matches("echo-barrier\n").count() >= 2
    });
    // Keep every line, including empty ones: a second Enter or retry must fail.
    // Terminal-driver and cat output may arrive in either order.
    let mut lines: Vec<_> = output.lines().collect();
    lines.sort_unstable();
    assert_eq!(
        lines,
        [
            "echo-barrier",
            "echo-barrier",
            "one delivery",
            "one delivery"
        ]
    );
    lab.event(
        "PreToolUse",
        json!({"session_id":"main","tool_name":"post-turn"}),
    );
    lab.event(
        "Notification",
        json!({"session_id":"main","notification_type":"idle_prompt"}),
    );
    assert_eq!(lab.json(&["status", "lab/main"])["state"], "idle");
}
#[test]
fn state_started_tracks_transitions_and_survives_events_and_cursor_replay() {
    let lab = Lab::new();
    lab.recognized();
    let initial = lab.json(&["status", "lab/main"]);
    assert!(initial["state_started"].is_null());
    let dir = lab.root.path().join("pens/lab/main");
    for (t, ev, extra, state, since, turn) in [
        (
            10,
            "SessionStart",
            json!({"cwd":"/tmp","has_transcript":true}),
            "idle",
            10,
            None,
        ),
        (
            20,
            "UserPromptSubmit",
            json!({"prompt":"first"}),
            "working",
            20,
            Some(20),
        ),
        (
            30,
            "PreToolUse",
            json!({"tool_name":"shell"}),
            "working",
            20,
            Some(20),
        ),
        (40, "PermissionRequest", json!({}), "blocked", 40, Some(20)),
        (50, "PermissionRequest", json!({}), "blocked", 40, Some(20)),
        (
            60,
            "Notification",
            json!({"notification_type":"permission_prompt"}),
            "blocked",
            40,
            Some(20),
        ),
        (
            61,
            "SessionStart",
            json!({"cwd":"/tmp","has_transcript":true,"source":"compact"}),
            "blocked",
            40,
            Some(20),
        ),
        (
            62,
            "SessionStart",
            json!({"session_id":"sub","cwd":"/tmp"}),
            "blocked",
            40,
            Some(20),
        ),
        (
            63,
            "Stop",
            json!({"session_id":"sub"}),
            "blocked",
            40,
            Some(20),
        ),
        (70, "PostToolUse", json!({}), "working", 70, Some(20)),
        (
            80,
            "Stop",
            json!({"background_running":0}),
            "idle",
            80,
            Some(20),
        ),
        (
            85,
            "Stop",
            json!({"background_running":0}),
            "idle",
            80,
            Some(20),
        ),
        (
            90,
            "Notification",
            json!({"notification_type":"idle_prompt"}),
            "idle",
            80,
            Some(20),
        ),
        (
            100,
            "UserPromptSubmit",
            json!({"prompt":"next"}),
            "working",
            100,
            Some(100),
        ),
        (
            110,
            "PermissionRequest",
            json!({}),
            "blocked",
            110,
            Some(100),
        ),
        (120, "Interrupt", json!({}), "idle", 120, Some(100)),
        (130, "PreToolUse", json!({}), "working", 130, Some(100)),
        (
            140,
            "Notification",
            json!({"notification_type":"idle_prompt"}),
            "idle",
            140,
            Some(100),
        ),
        (150, "StopFailure", json!({}), "idle", 140, Some(100)),
        (160, "SessionEnd", json!({}), "exiting", 160, Some(100)),
    ] {
        let mut event = json!({"v":1,"inst":initial["instance"],"session_id":"main","t":t,"ev":ev});
        event
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        writeln!(
            fs::OpenOptions::new()
                .append(true)
                .open(dir.join("events"))
                .unwrap(),
            "{event}"
        )
        .unwrap();
        for public in [
            lab.json(&["status", "lab/main"]),
            lab.json(&["status", "lab/main"]),
        ] {
            assert_eq!(public["state"], state, "{ev}");
            assert_eq!(public["state_started"], since, "{ev}");
            assert_eq!(public["turn_started"], json!(turn), "{ev}");
        }
        // Old cursors must replay even if they retain a now-stale extension field.
        let mut cursor: Value =
            serde_json::from_slice(&fs::read(dir.join("cursor")).unwrap()).unwrap();
        cursor["cursor"] = json!(2);
        cursor["state_started"] = json!(1);
        fs::write(dir.join("cursor"), cursor.to_string()).unwrap();
        assert_eq!(
            lab.json(&["status", "lab/main"])["state_started"],
            since,
            "replay {ev}"
        );
    }
}

#[test]
fn hook_is_fail_open_and_sandbox_public_commands_leave_no_state() {
    let lab = Lab::new();
    let o = lab
        .cmd()
        .args(["status", "missing"])
        .env("CODEX_SANDBOX", "yes")
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(6));
    assert!(!lab.root.path().join("pens").exists());
    let o = lab
        .cmd()
        .args(["__hook", "Stop"])
        .env("CORRAL_EVENTS", lab.root.path().join("absent/events"))
        .output()
        .unwrap();
    assert!(o.status.success());
    assert!(o.stdout.is_empty());
    assert!(
        lab.cmd()
            .arg("--version")
            .env("CODEX_SANDBOX", "yes")
            .output()
            .unwrap()
            .status
            .success()
    );
    for name in [
        "../escape",
        "a//b",
        "lock",
        "a/sock",
        "stop.lock",
        "a/stop.lock",
    ] {
        let out = lab.out(&["start", name, "--", "/bin/cat"]);
        assert_eq!(out.status.code(), Some(1));
        assert_eq!(
            serde_json::from_slice::<Value>(&out.stdout).unwrap()["error"],
            "bad_name"
        );
    }
}
#[test]
fn injected_hooks_use_the_namespace_entry_and_adapter_arguments_are_preserved() {
    let lab = Lab::new();
    let fake = lab.root.path().join("claude");
    fs::write(
        &fake,
        "#!/bin/sh\nprintf '%s\\0' \"$@\" > \"$CORRAL_EVENTS.args\"\nexec /bin/cat\n",
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();
    let public = lab.root.path().join("current-corral");
    std::os::unix::fs::symlink(&lab.core, &public).unwrap();
    let started = Command::new(&public)
        .env("HOME", lab.root.path())
        .env("CORRAL_HOME", lab.root.path().join("pens"))
        .env("SHELL", "/bin/sh")
        .env_remove("CODEX_SANDBOX")
        .args([
            "start",
            "lab/main",
            "--prompt",
            "-literal prompt",
            "--",
            fake.to_str().unwrap(),
            "--model",
            "fixture",
        ])
        .output()
        .unwrap();
    assert!(
        started.status.success(),
        "{}",
        String::from_utf8_lossy(&started.stdout)
    );
    // An unrelated command link does not change the namespace's helper entry.
    fs::remove_file(&public).unwrap();
    std::os::unix::fs::symlink("/usr/bin/false", &public).unwrap();
    let args_file = lab.root.path().join("pens/lab/main/events.args");
    eventually(|| args_file.exists());
    let bytes = fs::read(&args_file).unwrap();
    let args: Vec<_> = bytes
        .split(|c| *c == 0)
        .filter(|s| !s.is_empty())
        .map(|b| std::str::from_utf8(b).unwrap())
        .collect();
    assert_eq!(&args[2..], &["--model", "fixture", "--", "-literal prompt"]);
    let settings: Value = serde_json::from_str(args[1]).unwrap();
    let hook = settings["hooks"]["SessionStart"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    assert!(
        hook.contains(
            lab.root
                .path()
                .join("pens/.runtime/helper")
                .to_str()
                .unwrap()
        ),
        "helper: {hook}"
    );
    assert!(!hook.contains("python"));
    let next = lab.root.path().join("next-corral");
    common::copy_exe(&lab.core, &next);
    assert_eq!(
        lab.json(&["upgrade", "lab/main", "--exe", next.to_str().unwrap()])["result"],
        "complete"
    );
    assert_eq!(
        fs::read_link(lab.root.path().join("pens/.runtime/helper")).unwrap(),
        fs::canonicalize(&next).unwrap()
    );
    let instance = lab.json(&["status", "lab/main"])["instance"]
        .as_str()
        .unwrap()
        .to_owned();
    // Execute exactly the command handed to the agent, not a parallel test-only adapter.
    let mut child = Command::new("/bin/sh");
    child
        .args(["-c", hook])
        .env(
            "CORRAL_EVENTS",
            lab.root.path().join("pens/lab/main/events"),
        )
        .env("CORRAL_INSTANCE", instance)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped());
    let mut proc = child.spawn().unwrap();
    serde_json::to_writer(
        proc.stdin.take().unwrap(),
        &json!({"session_id":"main","cwd":lab.root.path(),"transcript_path":"transcript"}),
    )
    .unwrap();
    let o = proc.wait_with_output().unwrap();
    assert!(o.status.success());
    assert!(o.stdout.is_empty());
    // This test started in the inherited cwd; the hook's independent output is still v1.
    let records = fs::read_to_string(lab.root.path().join("pens/lab/main/events")).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(records.trim()).unwrap()["v"],
        1
    );
}

#[test]
fn raw_v2_events_keep_original_input_reply_and_retry_grace_with_v1() {
    let lab = Lab::new();
    lab.recognized();
    let instance = lab.json(&["status", "lab/main"])["instance"].clone();
    let emit = |ev: &str, event: Value| {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        let record = json!({"v":2,"inst":instance,"adapter":"omp","ev":ev,"t":t,"session_id":"main","cwd":"/tmp","has_ui":true,"event":event});
        writeln!(
            fs::OpenOptions::new()
                .append(true)
                .open(lab.root.path().join("pens/lab/main/events"))
                .unwrap(),
            "{record}"
        )
        .unwrap();
    };
    emit("session_start", json!({"reason":"startup"}));
    assert_eq!(lab.json(&["status", "lab/main"])["state"], "idle");
    std::thread::scope(|scope| {
        let sending = scope.spawn(|| lab.json(&["send", "lab/main", "original", "--timeout", "2"]));
        eventually(|| {
            String::from_utf8_lossy(&lab.out(&["read", "lab/main"]).stdout).contains("original")
        });
        emit("input", json!({"text":"original"}));
        emit("before_agent_start", json!({"prompt":"expanded"}));
        assert_eq!(sending.join().unwrap()["confirmed"], true);
    });
    assert_eq!(lab.json(&["status", "lab/main"])["state"], "working");
    emit(
        "message_end",
        json!({"message":{"role":"assistant","content":[{"type":"text","text":"raw reply"}]}}),
    );
    emit(
        "agent_end",
        json!({"messages":[{"role":"assistant","stopReason":"error","errorMessage":"503 unavailable"}]}),
    );
    std::thread::sleep(Duration::from_millis(350));
    assert_eq!(lab.json(&["status", "lab/main"])["state"], "working");
    emit("agent_start", json!({}));
    emit("agent_end", json!({"willContinue":true}));
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(lab.json(&["status", "lab/main"])["state"], "working");
    emit("agent_end", json!({}));
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(lab.json(&["reply", "lab/main"])["text"], "raw reply");
    assert_eq!(lab.json(&["status", "lab/main"])["state"], "idle");
    lab.event(
        "PreToolUse",
        json!({"session_id":"main","tool_name":"v1-tool"}),
    );
    assert_eq!(lab.json(&["status", "lab/main"])["last_tool"], "v1-tool");
}

#[test]
fn agent_exit_drains_last_output_to_attached_client() {
    let lab = Lab::new();
    lab.json(&[
        "start",
        "lab/main",
        "--",
        "/bin/sh",
        "-c",
        "read line; head -c 262144 /dev/zero; printf FINAL_MARKER",
    ]);
    let (mut socket, _) = lab.attach(24, 80);
    input(&mut socket, b'i', b"go\r");
    let mut bytes = Vec::new();
    socket.read_to_end(&mut bytes).unwrap();
    assert!(
        bytes.ends_with(b"FINAL_MARKER"),
        "final agent output lost; received {} bytes",
        bytes.len()
    );
    eventually(|| lab.out(&["status", "lab/main"]).status.code() == Some(2));
}

#[test]
fn stop_does_not_reuse_exit_record_after_same_name_restart() {
    use std::io::BufRead;
    use std::os::unix::net::UnixListener;

    let lab = Lab::new();
    let dir = lab.root.path().join("pens/lab/main");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("meta.json"), r#"{"kind":"kimi"}"#).unwrap();
    for instance in ["previous", "current"] {
        let listener = UnixListener::bind(dir.join("sock")).unwrap();
        let lock = hold_stop_test_lock(&dir.join("lock"));
        let dir = dir.clone();
        let server = std::thread::spawn(move || {
            // Synthetic pen transport, with the second incarnation deliberately
            // leaving the previous incarnation's exit record on disk.
            for op in ["status", "stop"] {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut line = String::new();
                std::io::BufReader::new(&socket)
                    .read_line(&mut line)
                    .unwrap();
                let request: Value = serde_json::from_str(&line).unwrap();
                assert_eq!(request["op"], op);
                writeln!(
                    socket,
                    "{}",
                    json!({"ok":true,"proto":1,"instance":instance})
                )
                .unwrap();
            }
            if instance == "previous" {
                fs::write(
                    dir.join("exit.json"),
                    json!({"instance":instance,"code":0,"stop_step":"SIGKILL","remaining_pids":[12345]}).to_string(),
                ).unwrap();
            }
            drop(listener);
            fs::remove_file(dir.join("sock")).unwrap();
            drop(lock);
        });
        let output = lab.out(&["stop", "lab/main", "--timeout", "4"]);
        server.join().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let stopped: Value = serde_json::from_slice(&output.stdout).unwrap();
        if instance == "previous" {
            assert_eq!(
                stopped,
                json!({"ok":true,"name":"lab/main","instance":"previous",
                "exit_code":0,"stopped_by":"SIGKILL","remaining_pids":[12345]})
            );
        } else {
            assert_eq!(
                stopped,
                json!({"ok":true,"name":"lab/main","instance":"current",
                "exit_code":null,"stopped_by":null})
            );
        }
    }
}

fn hold_stop_test_lock(path: &std::path::Path) -> fs::File {
    use std::os::fd::AsRawFd;
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .unwrap();
    assert_eq!(
        unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    file
}

#[test]
fn stop_lock_left_by_old_ls_allows_restart_and_eventual_cleanup() {
    use std::os::unix::fs::MetadataExt;

    let lab = Lab::new();
    let dir = lab.root.path().join("pens/lab/main");
    fs::create_dir_all(&dir).unwrap();
    // This is what old ls leaves: the name lock and all records are gone,
    // but it does not know about the lock still held by a new stop.
    let stop_lock = hold_stop_test_lock(&dir.join("stop.lock"));
    let inode = stop_lock.metadata().unwrap().ino();
    assert_eq!(lab.json(&["ls"])["agents"], json!([]));
    assert!(dir.join("stop.lock").is_file());
    lab.json(&["start", "lab/main", "--", "/bin/cat"]);
    assert_eq!(fs::metadata(dir.join("stop.lock")).unwrap().ino(), inode);
    drop(stop_lock);
    let stopped = lab.json(&["stop", "lab/main", "--timeout", "4"]);
    assert_eq!(stopped["stopped_by"], "SIGHUP");
    assert!(!dir.join("stop.lock").exists());
    assert_eq!(lab.json(&["ls"])["agents"], json!([]));
    assert!(!dir.exists());

    // A stop that dies leaves the same marker with no flock owner. No restart
    // should be necessary for new ls to find and clean this orphan directory.
    fs::create_dir_all(&dir).unwrap();
    drop(hold_stop_test_lock(&dir.join("stop.lock")));
    assert_eq!(lab.json(&["ls"])["agents"], json!([]));
    assert!(!dir.exists());
}

#[test]
fn stop_timeout_does_not_cancel_pen_owned_shutdown() {
    let lab = Lab::new();
    lab.json(&[
        "start",
        "lab/main",
        "--",
        "/bin/sh",
        "-c",
        "trap '' HUP; printf READY; while :; do read x; done",
    ]);
    eventually(|| {
        String::from_utf8_lossy(&lab.out(&["read", "lab/main"]).stdout).contains("READY")
    });
    let output = lab.out(&["stop", "lab/main", "--timeout", "0"]);
    assert_eq!(output.status.code(), Some(4));
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        if lab.out(&["status", "lab/main"]).status.code() == Some(2) {
            break;
        }
        assert!(Instant::now() < end, "pen did not finish stop sequence");
        std::thread::sleep(Duration::from_millis(30));
    }
    let again = lab.json(&["start", "lab/main", "--", "/bin/cat"]);
    assert_eq!(again["ok"], true);
}

#[test]
fn environment_is_rebuilt_then_overridden_without_parent_identity_leaks() {
    let lab = Lab::new();
    let shell = lab.root.path().join("login-shell");
    fs::write(&shell,"#!/bin/sh\nprintf '\\n__CORRAL_ENV_BEGIN__\\nPATH=/usr/bin:/bin\\0HOME=%s\\0CODEX_SESSION_ID=old\\0CODEX_HOME=/test/config\\0WEZTERM_UNIX_SOCKET=old\\0CLAUDECODE=old\\0FROM_LOGIN=yes\\0\\n__CORRAL_ENV_END__\\n' \"$HOME\"\n").unwrap();
    fs::set_permissions(&shell, fs::Permissions::from_mode(0o700)).unwrap();
    let target = lab.root.path().join("environment");
    let o = lab
        .cmd()
        .env("SHELL", shell)
        .env("PARENT_ONLY", "no-leak")
        .args([
            "start",
            "lab/main",
            "--env",
            "FROM_LOGIN=override",
            "--env",
            "CORRAL_NAME=wrong",
            "--",
            "/bin/sh",
            "-c",
            // Rename into place: the redirect creates the file before env writes it.
            "/usr/bin/env > \"$1.tmp\"; mv \"$1.tmp\" \"$1\"; exec /bin/cat",
            "fixture",
            target.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    eventually(|| target.exists());
    let text = fs::read_to_string(target).unwrap();
    assert!(text.contains("FROM_LOGIN=override\n"));
    assert!(text.contains("CODEX_HOME=/test/config\n"));
    assert!(text.contains("CORRAL_NAME=lab/main\n"));
    for forbidden in [
        "PARENT_ONLY=",
        "CODEX_SESSION_ID=",
        "CLAUDECODE=",
        "WEZTERM_UNIX_SOCKET=",
    ] {
        assert!(!text.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn start_preserves_existing_directory_permissions() {
    let lab = Lab::new();
    let home = lab.root.path().join("pens");
    let group = home.join("lab");
    fs::create_dir_all(&group).unwrap();
    fs::set_permissions(&home, fs::Permissions::from_mode(0o775)).unwrap();
    fs::set_permissions(&group, fs::Permissions::from_mode(0o755)).unwrap();
    lab.json(&["start", "lab/main", "--", "/bin/cat"]);
    for (path, mode) in [
        (home, 0o775),
        (group.clone(), 0o755),
        (group.join("main"), 0o700),
    ] {
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            mode,
            "{}",
            path.display()
        );
    }
}

#[test]
fn concurrent_unique_starts_and_listing_preserve_live_locks() {
    let lab = Lab::new();
    let names = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..3)
            .map(|_| scope.spawn(|| lab.json(&["start", "lab/main", "--unique", "--", "/bin/cat"])))
            .collect();
        workers
            .into_iter()
            .map(|w| w.join().unwrap()["name"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    });
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(sorted, ["lab/main-1", "lab/main-2", "lab/main-3"]);
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..3)
            .map(|_| {
                scope.spawn(|| {
                    assert_eq!(lab.json(&["ls"])["agents"].as_array().unwrap().len(), 3);
                    for name in &names {
                        lab.json(&["status", name]);
                    }
                })
            })
            .collect();
        for w in workers {
            w.join().unwrap();
        }
    });
    for name in names {
        lab.json(&["stop", &name, "--timeout", "4"]);
    }
    assert_eq!(lab.json(&["ls"])["agents"], json!([]));
}

#[test]
fn deferred_send_cannot_follow_a_reused_name_to_a_new_instance() {
    let lab = Lab::new();
    lab.recognized();
    lab.event(
        "SessionStart",
        json!({"session_id":"main","cwd":"/tmp","transcript_path":"x"}),
    );
    lab.event(
        "UserPromptSubmit",
        json!({"session_id":"main","prompt":"working"}),
    );
    let old = lab.json(&["status", "lab/main"])["instance"].clone();
    let pending = lab.json(&[
        "send",
        "lab/main",
        "must-not-arrive",
        "--after",
        "lab/main",
        "--timeout",
        "1",
    ]);
    assert_eq!(pending["pending"], true);
    lab.json(&["stop", "lab/main", "--timeout", "4"]);
    let new = lab.json(&["start", "lab/main", "--", "/bin/cat"]);
    assert_ne!(old, new["instance"]);
    std::thread::sleep(Duration::from_millis(1500));
    assert!(
        !String::from_utf8_lossy(&lab.out(&["read", "lab/main"]).stdout)
            .contains("must-not-arrive")
    );
}

/// Runs corral with `args`, leaving it the write end of a pipe, as a caller on macOS does when
/// another of its threads is creating that pipe; returns the read end once corral returned.
fn leaking(lab: &Lab, args: &[&str]) -> std::io::PipeReader {
    use std::os::{fd::AsRawFd, unix::process::CommandExt};
    let (reader, writer) = std::io::pipe().unwrap();
    let fd = writer.as_raw_fd();
    let mut c = lab.cmd();
    c.args(args);
    unsafe {
        c.pre_exec(move || {
            if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let o = c.output().unwrap();
    assert!(
        o.status.success(),
        "{args:?} {}",
        String::from_utf8_lossy(&o.stdout)
    );
    reader
}
/// Whether every writer of the pipe is gone within a few seconds.
fn closes(reader: &std::io::PipeReader) -> bool {
    use std::os::fd::AsRawFd;
    let mut fd = libc::pollfd {
        fd: reader.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let mut b = [0u8; 1];
    unsafe { libc::poll(&mut fd, 1, 3000) == 1 && libc::read(fd.fd, b.as_mut_ptr().cast(), 1) == 0 }
}

#[test]
fn background_processes_do_not_keep_what_their_caller_leaked() {
    let lab = Lab::new();
    let path = lab.root.path().join("claude");
    fs::write(&path, "#!/bin/sh\nexec /bin/cat\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    let start = leaking(
        &lab,
        &[
            "start",
            "lab/main",
            "--cwd",
            "/tmp",
            "--",
            path.to_str().unwrap(),
        ],
    );
    assert!(
        closes(&start),
        "the pen or its agent kept the caller's pipe"
    );
    lab.event(
        "SessionStart",
        json!({"session_id":"main","cwd":"/tmp","transcript_path":"x"}),
    );
    lab.event(
        "UserPromptSubmit",
        json!({"session_id":"main","prompt":"working"}),
    );
    let send = leaking(
        &lab,
        &[
            "send",
            "lab/main",
            "later",
            "--after",
            "lab/main",
            "--timeout",
            "30",
        ],
    );
    assert!(closes(&send), "the deferred send kept the caller's pipe");
}
