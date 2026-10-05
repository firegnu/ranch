// From Saddle `crates/corral-core/tests/upgrade.rs` at commit `a31dea2`, unchanged.
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    process::{Command, Output},
    time::{Duration, Instant},
};

struct Lab {
    root: tempfile::TempDir,
    old: PathBuf,
    new: PathBuf,
    pids: std::sync::Mutex<Vec<i32>>,
}
impl Lab {
    fn new() -> Self {
        let root = tempfile::tempdir_in("/tmp").unwrap();
        let old = root.path().join("v1/corral");
        let new = root.path().join("v2/corral");
        for p in [&old, &new] {
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::copy(env!("CARGO_BIN_EXE_corral"), p).unwrap();
        }
        Self {
            root,
            old,
            new,
            pids: std::sync::Mutex::new(Vec::new()),
        }
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(&self.old)
            .args(args)
            .env("HOME", self.root.path())
            .env("CORRAL_HOME", self.root.path().join("pens"))
            .env_remove("CODEX_SANDBOX")
            .output()
            .unwrap()
    }
    fn json(&self, args: &[&str]) -> Value {
        let o = self.run(args);
        assert!(
            o.status.success(),
            "{args:?}: {} {}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        serde_json::from_slice(&o.stdout).unwrap()
    }
    fn connect(&self) -> UnixStream {
        let s = UnixStream::connect(self.root.path().join("pens/test/raw/sock")).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        s
    }
    fn request(&self, v: Value) -> Value {
        let mut s = self.connect();
        writeln!(s, "{v}").unwrap();
        let result = line(&mut s);
        self.track(&result);
        result
    }
    fn track(&self, value: &Value) {
        for pid in [
            &value["pen_pid"],
            &value["agent_pid"],
            &value["upgrade"]["backup_pid"],
        ]
        .into_iter()
        .filter_map(|v| v.as_i64())
        {
            let mut pids = self.pids.lock().unwrap();
            if !pids.contains(&(pid as i32)) {
                pids.push(pid as i32);
            }
        }
    }
    fn start(&self) {
        self.json(&["start", "test/raw", "--cwd", "/tmp", "--", "/bin/cat"]);
    }
    fn shim(&self, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = self.root.path().join("broken-image");
        fs::write(
            &path,
            format!(
                "#!/bin/sh\nif [ \"$1\" = __pen-probe ]; then exec '{}' \"$@\"; fi\n{body}\n",
                self.old.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }
    fn known(&self) {
        use std::os::unix::fs::PermissionsExt;
        let p = self.root.path().join("claude");
        fs::write(&p, "#!/bin/sh\nexec /bin/cat\n").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        self.json(&[
            "start",
            "test/raw",
            "--cwd",
            "/tmp",
            "--",
            p.to_str().unwrap(),
        ]);
        self.event("SessionStart", json!({"cwd":"/tmp","has_transcript":true}));
    }
    fn event(&self, ev: &str, extra: Value) {
        let instance = self.request(json!({"op":"status"}))["instance"].clone();
        let mut e = json!({"v":1,"inst":instance,"session_id":"main","t":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64(),"ev":ev});
        e.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        writeln!(
            fs::OpenOptions::new()
                .append(true)
                .open(self.root.path().join("pens/test/raw/events"))
                .unwrap(),
            "{e}"
        )
        .unwrap();
    }
    fn record(&self, id: &str) -> Value {
        self.json(&["after", "test/raw", "--request-id", id])["records"][0].clone()
    }
}
impl Drop for Lab {
    fn drop(&mut self) {
        let out = self.run(&["stop", "test/raw", "--timeout", "5"]);
        if !out.status.success() {
            for file in ["upgrade.json", "upgrade.committed"] {
                if let Ok(b) = fs::read(self.root.path().join("pens/test/raw").join(file))
                    && let Ok(v) = serde_json::from_slice::<Value>(&b)
                {
                    self.track(&json!({"upgrade":v["pen"]["upgrade"]}));
                }
            }
            for &pid in self.pids.lock().unwrap().iter() {
                unsafe {
                    libc::kill(pid, libc::SIGKILL);
                }
            }
        }
    }
}
fn line(s: &mut UnixStream) -> Value {
    try_line(s).unwrap()
}
fn try_line(s: &mut UnixStream) -> std::io::Result<Value> {
    let mut bytes = Vec::new();
    loop {
        let mut b = [0];
        s.read_exact(&mut b)?;
        if b[0] == b'\n' {
            break;
        }
        bytes.push(b[0]);
    }
    Ok(serde_json::from_slice(&bytes)?)
}

#[test]
fn public_upgrade_keeps_process_identity_and_an_incomplete_connection() {
    let lab = Lab::new();
    lab.start();
    let before = lab.request(json!({"op":"status"}));
    let mut partial = lab.connect();
    partial.write_all(b"{\"op\":\"sta").unwrap();
    // The partial request is already readable before the upgrade request.
    std::thread::sleep(Duration::from_millis(100));
    let result = lab.json(&["upgrade", "test/raw", "--exe", lab.new.to_str().unwrap()]);
    assert_eq!(result["result"], "complete", "{result}");
    partial.write_all(b"tus\"}\n").unwrap();
    let after = line(&mut partial);
    for key in ["agent_pid", "pen_pid", "instance", "started"] {
        assert_eq!(before[key], after[key], "{key}");
    }
    assert_eq!(after["exe"], json!(fs::canonicalize(&lab.new).unwrap()));
    assert_eq!(after["custody"], "owner");
    assert_eq!(after["upgrade"]["state"], "none");
    let sent = lab.json(&["send", "test/raw", "after upgrade"]);
    assert_eq!(sent["confirmed"], false);
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        if String::from_utf8_lossy(&lab.run(&["read", "test/raw"]).stdout).contains("after upgrade")
        {
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn queued_send_keeps_request_evidence_and_receipt_across_upgrade() {
    use base64::{Engine, engine::general_purpose::STANDARD as B64};
    let lab = Lab::new();
    lab.start();
    let mut sending = lab.connect();
    writeln!(sending,"{}",json!({"op":"send","request_id":"queued-1","chunks":[{"data":B64.encode(b"first"),"delay":1.5},{"data":B64.encode(b"second\r")}]})).unwrap();
    let until = Instant::now() + Duration::from_secs(2);
    loop {
        let st = lab.request(json!({"op":"status"}));
        if !st["recent_sends"].as_array().unwrap().is_empty() {
            assert_eq!(st["recent_sends"][0]["request_id"], "queued-1");
            assert_eq!(st["recent_sends"][0]["state"], "accepted");
            break;
        }
        assert!(Instant::now() < until);
    }
    assert_eq!(
        lab.json(&["upgrade", "test/raw", "--exe", lab.new.to_str().unwrap()])["result"],
        "complete"
    );
    assert_eq!(line(&mut sending)["ok"], true);
    let st = lab.request(json!({"op":"status"}));
    assert_eq!(st["recent_sends"][0]["request_id"], "queued-1");
    assert_eq!(st["recent_sends"][0]["state"], "written");
    assert_eq!(
        lab.json(&["stop", "test/raw", "--timeout", "5"])["exit_code"],
        -libc::SIGHUP
    );
}

#[test]
fn pre_io_crash_is_recovered_by_standby_without_restarting_the_agent() {
    let lab = Lab::new();
    lab.start();
    let before = lab.request(json!({"op":"status"}));
    let bad = lab.shim("kill -KILL $$");
    let result = lab.json(&["upgrade", "test/raw", "--exe", bad.to_str().unwrap()]);
    assert_eq!(result["result"], "failed", "{result}");
    let after = lab.request(json!({"op":"status"}));
    assert_eq!(after["custody"], "observer");
    assert_eq!(after["agent_pid"], before["agent_pid"]);
    assert_ne!(after["pen_pid"], before["pen_pid"]);
    assert_eq!(after["instance"], before["instance"]);
    lab.json(&["send", "test/raw", "standby works"]);
    assert!(lab.json(&["stop", "test/raw", "--timeout", "5"])["exit_code"].is_null());
}

#[test]
fn rejected_fd_identity_survives_fallback_recover_and_standby() {
    rejected_fd_identity(false);
}

#[test]
fn rejected_fd_identity_survives_backup_exit_and_recover() {
    rejected_fd_identity(true);
}

fn rejected_fd_identity(lose_backup: bool) {
    let lab = Lab::new();
    lab.start();
    let before = lab.request(json!({"op":"status"}));
    let dir = lab.root.path().join("pens/test/raw");
    let mut partial = lab.connect();
    partial.write_all(b"{\"op\":\"sta").unwrap();
    std::thread::sleep(Duration::from_millis(100));
    // Replace a client fd only in the exec'd process. K still holds the exact
    // original socket. Keep a copy of the pre-fault snapshot for comparison.
    let bad = lab.shim(&format!(
        r#"cp "$2/upgrade.json" "$2/baseline.json"
fd=$(sed -n 's/.*"sock":\([0-9][0-9]*\).*/\1/p' "$2/baseline.json")
test -n "$fd" || exit 1
eval "exec $fd<>\"$2/replaced-fd\""
exec '{}' "$@""#,
        lab.old.display()
    ));
    let mut request = lab.connect();
    writeln!(
        request,
        "{}",
        json!({"op":"upgrade","exe":bad,"epoch":"rejected-fd"})
    )
    .unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    while !dir.join("baseline.json").exists() {
        assert!(Instant::now() < until, "replacement image did not start");
        std::thread::sleep(Duration::from_millis(20));
    }
    let wait_hold = |attempt, state| {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let mut s = lab.connect();
            s.set_read_timeout(Some(Duration::from_millis(200)))
                .unwrap();
            writeln!(s, "{}", json!({"op":"status"})).unwrap();
            if let Ok(st) = try_line(&mut s) {
                lab.track(&st);
                assert_ne!(
                    st["upgrade"]["state"], "none",
                    "fd rejection was waived: {st}"
                );
                if st["upgrade"]["state"] == state && st["upgrade"]["attempt"] == attempt {
                    assert!(
                        st["upgrade"]["last_error"]
                            .as_str()
                            .unwrap()
                            .contains("inherited fd identity mismatch"),
                        "{st}"
                    );
                    assert_eq!(st["custody"], "handoff");
                    assert_eq!(st["agent_pid"], before["agent_pid"]);
                    return st;
                }
            }
            assert!(
                Instant::now() < until,
                "did not enter Hold for attempt {attempt}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    let held = wait_hold(1, "hold");
    let original: Value =
        serde_json::from_slice(&fs::read(dir.join("baseline.json")).unwrap()).unwrap();
    let assert_snapshot = |backup_removed| {
        assert!(!dir.join("upgrade.active").exists());
        let mut saved: Value =
            serde_json::from_slice(&fs::read(dir.join("upgrade.json")).unwrap()).unwrap();
        let mut expected = original.clone();
        if backup_removed {
            let pipes = [
                original["pen"]["upgrade"]["control"].clone(),
                original["pen"]["upgrade"]["alive"].clone(),
            ];
            expected["descriptors"]
                .as_array_mut()
                .unwrap()
                .retain(|d| !pipes.contains(&d["fd"]));
            for key in ["control", "alive", "backup_pid"] {
                assert!(saved["pen"]["upgrade"][key].is_null());
            }
        }
        saved["pen"].as_object_mut().unwrap().remove("upgrade");
        expected["pen"].as_object_mut().unwrap().remove("upgrade");
        assert_eq!(
            saved, expected,
            "failure recording changed the original identity or stream state"
        );
        assert!(fs::read(dir.join("replaced-fd")).unwrap().is_empty());
    };
    assert_snapshot(false);
    partial.write_all(b"tus\"}\n").unwrap();
    partial
        .set_read_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    assert!(
        try_line(&mut partial).is_err(),
        "Hold consumed a frozen client"
    );
    let expected_state = if lose_backup {
        assert_eq!(
            unsafe {
                libc::kill(
                    held["upgrade"]["backup_pid"].as_i64().unwrap() as i32,
                    libc::SIGKILL,
                )
            },
            0
        );
        wait_hold(1, "hold_unprotected");
        "hold_unprotected"
    } else {
        "hold"
    };
    let accepted = lab.request(json!({"op":"recover","exe":lab.new}));
    assert_eq!(accepted["result"], "accepted");
    let retried = wait_hold(2, expected_state);
    assert_eq!(retried["upgrade"]["epoch"], "rejected-fd");
    assert_snapshot(lose_backup);
    if lose_backup {
        assert_eq!(retried["upgrade"]["protected"], false);
        return;
    }
    assert_eq!(
        retried["upgrade"]["backup_pid"],
        held["upgrade"]["backup_pid"]
    );
    // Remove only this test's failed process. K must still be able to validate
    // the saved original identities and resume both frozen connections.
    assert_eq!(
        unsafe { libc::kill(before["pen_pid"].as_i64().unwrap() as i32, libc::SIGKILL) },
        0
    );
    partial
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let restored = line(&mut partial);
    lab.track(&restored);
    assert_eq!(restored["pen_pid"], held["upgrade"]["backup_pid"]);
    assert_eq!(restored["agent_pid"], before["agent_pid"]);
    assert_eq!(restored["instance"], before["instance"]);
    assert_eq!(line(&mut request)["result"], "accepted");
    let after = lab.request(json!({"op":"status"}));
    assert_eq!(after["custody"], "observer");
    assert_eq!(after["upgrade"]["state"], "none");
    assert_eq!(after["upgrade"]["attempt"], 2);
    lab.json(&["send", "test/raw", "original standby resources"]);
}

#[test]
fn hold_recovers_the_same_epoch_and_preserves_frozen_connections() {
    recover_from_hold(false, false);
}
#[test]
fn hold_reports_lost_standby_and_can_recover_unprotected() {
    recover_from_hold(true, false);
}
#[test]
fn recover_record_failure_is_reported_as_failed_and_can_be_retried() {
    recover_from_hold(false, true);
}
fn recover_from_hold(lose_backup: bool, fail_record: bool) {
    let lab = Lab::new();
    lab.start();
    let meta_path = lab.root.path().join("pens/test/raw/meta.json");
    let meta = fs::read(&meta_path).unwrap();
    let bad = lab.shim(&format!(
        "printf '{{\"instance\":\"invalid\"}}' > \"$2/meta.json\"\nexec '{}' \"$@\"",
        lab.old.display()
    ));
    let before = lab.request(json!({"op":"status"}));
    let mut request = lab.connect();
    writeln!(
        request,
        "{}",
        json!({"op":"upgrade","exe":bad,"epoch":"hold-epoch"})
    )
    .unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    let held = loop {
        let mut s = lab.connect();
        s.set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        writeln!(s, "{}", json!({"op":"status"})).unwrap();
        if let Ok(st) = try_line(&mut s) {
            lab.track(&st);
            if st["upgrade"]["state"] == "hold" {
                break st;
            }
        }
        assert!(
            Instant::now() < until,
            "Hold did not accept a fresh control connection"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    if lose_backup {
        assert_eq!(
            unsafe {
                libc::kill(
                    held["upgrade"]["backup_pid"].as_i64().unwrap() as i32,
                    libc::SIGKILL,
                )
            },
            0
        );
        loop {
            if lab.request(json!({"op":"status"}))["upgrade"]["state"] == "hold_unprotected" {
                break;
            }
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    assert_eq!(
        lab.request(json!({"op":"send","chunks":[]}))["error"],
        "hold"
    );
    if fail_record {
        use std::os::unix::fs::PermissionsExt;
        let dir = meta_path.parent().unwrap();
        fs::set_permissions(dir, fs::Permissions::from_mode(0o500)).unwrap();
        let failed = lab.json(&["recover", "test/raw", "--exe", lab.new.to_str().unwrap()]);
        let observed = lab.request(json!({"op":"status"}));
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(failed["result"], "failed");
        assert_eq!(observed["upgrade"]["result"], "failed");
        assert!(observed["upgrade"]["last_error"].is_string());
    }
    fs::write(meta_path, meta).unwrap();
    let result = lab.json(&["recover", "test/raw", "--exe", lab.new.to_str().unwrap()]);
    assert_eq!(result["result"], "complete", "{result}");
    assert_eq!(result["upgrade"]["epoch"], "hold-epoch");
    assert_eq!(
        result["upgrade"]["attempt"],
        if fail_record { 3 } else { 2 }
    );
    assert_eq!(line(&mut request)["result"], "accepted");
    let after = lab.request(json!({"op":"status"}));
    assert_eq!(before["agent_pid"], after["agent_pid"]);
    assert_eq!(before["pen_pid"], after["pen_pid"]);
}

#[test]
fn deferred_send_has_a_public_durable_result_and_request_identity() {
    let lab = Lab::new();
    lab.start();
    let pending = lab.json(&[
        "send",
        "test/raw",
        "durable reminder",
        "--after",
        "test/raw",
        "--timeout",
        "2",
    ]);
    let id = pending["request_id"]
        .as_str()
        .expect("persistent request identity");
    let until = Instant::now() + Duration::from_secs(4);
    loop {
        let records = lab.json(&["after", "test/raw", "--request-id", id]);
        let record = &records["records"][0];
        if record["phase"] == "sent" {
            assert_eq!(record["result"]["confirmed"], false);
            assert_eq!(record["request_id"], id);
            break;
        }
        assert!(Instant::now() < until, "{records}");
        std::thread::sleep(Duration::from_millis(20));
    }
    let result = lab.json(&["upgrade", "--all", "--exe", lab.new.to_str().unwrap()]);
    assert_eq!(result["ok"], true, "{result}");
    let out = lab.run(&["read", "test/raw"]);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout)
            .matches("durable reminder")
            .count(),
        2
    );
}

#[test]
fn waiting_reminder_hands_over_with_its_deadline_and_delivers_once() {
    let lab = Lab::new();
    lab.known();
    lab.event("UserPromptSubmit", json!({"prompt":"busy"}));
    let p = lab.json(&[
        "send",
        "test/raw",
        "queued notice",
        "--after",
        "test/raw",
        "--timeout",
        "8",
    ]);
    let id = p["request_id"].as_str().unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    let before = loop {
        let r = lab.record(id);
        if r["worker"]["pid"].is_number() {
            break r;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(20));
    };
    let result = lab.json(&["upgrade", "--all", "--exe", lab.new.to_str().unwrap()]);
    assert_eq!(result["ok"], true, "{result}");
    let after = lab.record(id);
    assert_eq!(after["phase"], "waiting");
    assert_eq!(after["wait_deadline"], before["wait_deadline"]);
    assert_ne!(after["worker"]["pid"], before["worker"]["pid"]);
    assert_eq!(
        after["worker"]["exe"],
        json!(fs::canonicalize(&lab.new).unwrap())
    );
    lab.event("Stop", json!({}));
    loop {
        if String::from_utf8_lossy(&lab.run(&["read", "test/raw"]).stdout).contains("queued notice")
        {
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(20));
    }
    lab.event("UserPromptSubmit", json!({"prompt":"queued notice"}));
    loop {
        let r = lab.record(id);
        if r["phase"] == "confirmed" {
            break;
        }
        assert!(Instant::now() < until, "{r}");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        String::from_utf8_lossy(&lab.run(&["read", "test/raw"]).stdout)
            .matches("queued notice")
            .count(),
        2
    );
}

#[test]
fn sending_with_evicted_evidence_becomes_unknown_without_replay() {
    use base64::{Engine, engine::general_purpose::STANDARD as B64};
    let lab = Lab::new();
    lab.start();
    let mut blocker = lab.connect();
    writeln!(blocker,"{}",json!({"op":"keys","chunks":[{"data":B64.encode(b""),"delay":2.0},{"data":B64.encode(b"\r")}]})).unwrap();
    let p = lab.json(&[
        "send",
        "test/raw",
        "uncertain reminder",
        "--after",
        "test/raw",
        "--timeout",
        "4",
    ]);
    let id = p["request_id"].as_str().unwrap();
    let until = Instant::now() + Duration::from_secs(6);
    loop {
        let r = lab.record(id);
        let st = lab.request(json!({"op":"status"}));
        if r["phase"] == "sending"
            && st["recent_sends"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["request_id"] == id)
        {
            assert_eq!(
                unsafe { libc::kill(r["worker"]["pid"].as_i64().unwrap() as i32, libc::SIGKILL) },
                0
            );
            break;
        }
        assert!(Instant::now() < until, "{r}");
    }
    let mut peers = Vec::new();
    for i in 0..11 {
        let mut c = lab.connect();
        writeln!(c,"{}",json!({"op":"send","request_id":format!("evict-{i}"),"chunks":[{"data":B64.encode(b"\r")}]})).unwrap();
        peers.push(c);
    }
    loop {
        if !lab.request(json!({"op":"status"}))["recent_sends"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["request_id"] == id)
        {
            break;
        }
        assert!(Instant::now() < until);
    }
    lab.json(&["upgrade", "test/raw", "--exe", lab.new.to_str().unwrap()]);
    let r = loop {
        let r = lab.record(id);
        if r["phase"] == "unknown" {
            break r;
        }
        assert!(Instant::now() < until, "{r}");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(r["request_id"], id);
    assert_eq!(line(&mut blocker)["ok"], true);
    for mut peer in peers {
        assert_eq!(line(&mut peer)["ok"], true);
    }
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        String::from_utf8_lossy(&lab.run(&["read", "test/raw"]).stdout)
            .matches("uncertain reminder")
            .count(),
        2
    );
}

#[test]
fn a_disconnected_send_receipt_cannot_close_a_reused_client_fd() {
    use base64::{Engine, engine::general_purpose::STANDARD as B64};
    let lab = Lab::new();
    lab.start();
    let mut sender = lab.connect();
    writeln!(sender,"{}",json!({"op":"send","request_id":"gone","chunks":[{"data":B64.encode(b"first"),"delay":0.6},{"data":B64.encode(b"\r")}]})).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    drop(sender);
    std::thread::sleep(Duration::from_millis(100));
    let mut partial = lab.connect();
    partial.write_all(b"{\"op\":\"sta").unwrap();
    lab.json(&["upgrade", "test/raw", "--exe", lab.new.to_str().unwrap()]);
    std::thread::sleep(Duration::from_millis(700));
    partial.write_all(b"tus\"}\n").unwrap();
    let st = line(&mut partial);
    assert!(
        st["agent_pid"].is_number(),
        "received somebody else's write receipt: {st}"
    );
}

fn frame(s: &mut UnixStream, bytes: &[u8]) {
    s.write_all(b"i").unwrap();
    s.write_all(&(bytes.len() as u32).to_be_bytes()).unwrap();
    s.write_all(bytes).unwrap();
}

#[test]
fn busy_output_partial_frames_terminal_modes_and_writer_order_survive_two_upgrades() {
    let lab = Lab::new();
    lab.json(&["start","test/raw","--cwd","/tmp","--","/bin/sh","-c",
        "stty -echo; read a; printf 'PARTIAL_MARKER\\033[?200'; read a; printf '4h\\033]2;kept\\007'; i=0; while [ $i -lt 100 ]; do printf 'SEQ:%03d\\n' $i; i=$((i+1)); sleep 0.005; done; printf END; exec /bin/cat"]);
    let mut first = lab.connect();
    writeln!(first, "{}", json!({"op":"attach","rows":33,"cols":91})).unwrap();
    assert_eq!(line(&mut first)["readonly"], false);
    let mut second = lab.connect();
    writeln!(second, "{}", json!({"op":"attach","rows":22,"cols":80})).unwrap();
    assert_eq!(line(&mut second)["readonly"], true);
    frame(&mut first, b"emit\r");
    let until = Instant::now() + Duration::from_secs(8);
    loop {
        if String::from_utf8_lossy(&lab.run(&["read", "test/raw"]).stdout)
            .contains("PARTIAL_MARKER")
        {
            break;
        }
        assert!(Instant::now() < until);
    }
    let before = lab.request(json!({"op":"status"}));
    first.write_all(b"i\0\0\0\x05ne").unwrap();
    assert_eq!(
        lab.json(&["upgrade", "test/raw", "--exe", lab.new.to_str().unwrap()])["result"],
        "complete"
    );
    let after = lab.request(json!({"op":"status"}));
    for key in [
        "last_human_input",
        "size",
        "attached",
        "agent_pid",
        "pen_pid",
    ] {
        assert_eq!(before[key], after[key], "{key}");
    }
    assert_eq!(
        lab.run(&["send", "test/raw", "protected"]).status.code(),
        Some(8)
    );
    first.write_all(b"xt\r").unwrap();
    loop {
        if String::from_utf8_lossy(&lab.run(&["read", "test/raw"]).stdout).contains("SEQ:010") {
            break;
        }
        assert!(Instant::now() < until);
    }
    assert_eq!(
        lab.json(&["upgrade", "test/raw", "--exe", lab.old.to_str().unwrap()])["result"],
        "complete"
    );
    let st = lab.request(json!({"op":"status"}));
    assert_eq!(st["bracketed_paste"], true);
    assert_eq!(st["title"], "kept");
    for reader in [&mut first, &mut second] {
        let mut bytes = Vec::new();
        loop {
            let mut b = [0; 8192];
            let n = reader.read(&mut b).unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&b[..n]);
            if bytes.ends_with(b"END") {
                break;
            }
            assert!(Instant::now() < until);
        }
        let text = String::from_utf8_lossy(&bytes);
        let positions: Vec<_> = (0..100)
            .map(|i| {
                let needle = format!("SEQ:{i:03}");
                assert_eq!(text.matches(&needle).count(), 1, "{needle}");
                text.find(&needle).unwrap()
            })
            .collect();
        assert!(positions.windows(2).all(|p| p[0] < p[1]));
    }
    drop(first);
    loop {
        let st = lab.request(json!({"op":"status"}));
        if st["attached"] == 1 {
            assert_eq!(st["size"], json!([22, 80]));
            break;
        }
        assert!(Instant::now() < until);
    }
    frame(&mut second, b"next writer\r");
    loop {
        if String::from_utf8_lossy(&lab.run(&["read", "test/raw"]).stdout).contains("next writer") {
            break;
        }
        assert!(Instant::now() < until);
    }
}

#[test]
fn batch_distinguishes_an_old_pen_without_sending_it_an_upgrade_or_stop() {
    use std::os::{fd::AsRawFd, unix::net::UnixListener};
    let lab = Lab::new();
    lab.known();
    lab.event("UserPromptSubmit", json!({"prompt":"busy"}));
    let dir = lab.root.path().join("pens/test/legacy");
    fs::create_dir_all(&dir).unwrap();
    let lock = fs::File::create(dir.join("lock")).unwrap();
    assert_eq!(
        unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    fs::write(
        dir.join("meta.json"),
        json!({"proto":1,"instance":"legacy","kind":"cat"}).to_string(),
    )
    .unwrap();
    let listener = UnixListener::bind(dir.join("sock")).unwrap();
    let fake = std::thread::spawn(move || {
        for _ in 0..3 {
            let (mut c, _) = listener.accept().unwrap();
            assert_eq!(line(&mut c)["op"], "status");
            writeln!(c, "{}", json!({"ok":true,"proto":1,"instance":"legacy"})).unwrap();
        }
    });
    let pending = lab.json(&[
        "send",
        "test/legacy",
        "new persistent reminder",
        "--after",
        "test/raw",
        "--timeout",
        "4",
    ]);
    let id = pending["request_id"].as_str().unwrap();
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        if lab.json(&["after", "test/legacy", "--request-id", id])["records"][0]["worker"]["pid"]
            .is_number()
        {
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(20));
    }
    let result = lab.json(&["upgrade", "--all", "--exe", lab.new.to_str().unwrap()]);
    fake.join().unwrap();
    assert_eq!(result["ok"], false);
    let entries = result["results"].as_array().unwrap();
    assert_eq!(
        entries.iter().find(|v| v["name"] == "test/raw").unwrap()["result"],
        "complete"
    );
    assert_eq!(
        entries.iter().find(|v| v["name"] == "test/legacy").unwrap()["result"],
        "needs_restart"
    );
    assert_eq!(
        lab.json(&["after", "test/legacy", "--request-id", id])["records"][0]["worker"]["exe"],
        json!(fs::canonicalize(&lab.new).unwrap())
    );
}

#[test]
fn incompatible_schema_is_rejected_before_freezing_service() {
    let lab = Lab::new();
    lab.start();
    let before = lab.request(json!({"op":"status"}));
    let bad = lab.shim("exit 1");
    fs::write(
        &bad,
        "#!/bin/sh\nprintf '{\"ok\":true,\"schema\":999}\\n'\n",
    )
    .unwrap();
    let reply = lab.request(json!({"op":"upgrade","exe":bad}));
    assert_eq!(reply["ok"], false);
    let after = lab.request(json!({"op":"status"}));
    assert_eq!(after["upgrade"]["state"], "none");
    assert_eq!(after["pen_pid"], before["pen_pid"]);
    lab.json(&["send", "test/raw", "still serving"]);
}

#[test]
fn standby_never_replays_a_snapshot_marked_active() {
    use base64::{Engine, engine::general_purpose::STANDARD as B64};
    let lab = Lab::new();
    let output = lab.root.path().join("agent-received");
    lab.json(&[
        "start",
        "test/raw",
        "--cwd",
        "/tmp",
        "--",
        "/bin/sh",
        "-c",
        "stty -echo; while IFS= read -r line; do printf '%s\\n' \"$line\" >> \"$1\"; done",
        "fixture",
        output.to_str().unwrap(),
    ]);
    lab.request(json!({"op":"status"}));
    let mut sender = lab.connect();
    writeln!(sender,"{}",json!({"op":"send","request_id":"must-not-replay","chunks":[{"data":B64.encode(b""),"delay":2.0},{"data":B64.encode(b"unreplayable\r")}]})).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let bad = lab.shim("mv \"$2/upgrade.json\" \"$2/upgrade.active\"\nkill -KILL $$");
    let mut upgrade = lab.connect();
    writeln!(upgrade, "{}", json!({"op":"upgrade","exe":bad})).unwrap();
    assert!(try_line(&mut upgrade).is_err());
    std::thread::sleep(Duration::from_millis(2500));
    assert_eq!(lab.run(&["status", "test/raw"]).status.code(), Some(2));
    assert!(
        fs::read_to_string(output).unwrap_or_default().is_empty(),
        "standby replayed input after active"
    );
}

#[test]
fn accepted_reminder_keeps_observing_after_the_receipt_wait_elapsed() {
    use base64::{Engine, engine::general_purpose::STANDARD as B64};
    use std::os::fd::AsRawFd;
    let lab = Lab::new();
    lab.start();
    let mut blocker = lab.connect();
    writeln!(blocker,"{}",json!({"op":"keys","chunks":[{"data":B64.encode(b""),"delay":2.5},{"data":B64.encode(b"\r")}]})).unwrap();
    let p = lab.json(&[
        "send",
        "test/raw",
        "accepted reminder",
        "--after",
        "test/raw",
        "--timeout",
        "4",
    ]);
    let id = p["request_id"].as_str().unwrap();
    let until = Instant::now() + Duration::from_secs(6);
    loop {
        let r = lab.record(id);
        let st = lab.request(json!({"op":"status"}));
        if r["phase"] == "sending"
            && st["recent_sends"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["request_id"] == id)
        {
            assert_eq!(
                unsafe { libc::kill(r["worker"]["pid"].as_i64().unwrap() as i32, libc::SIGKILL) },
                0
            );
            break;
        }
        assert!(Instant::now() < until);
    }
    // Synthetic crash recovery record with an elapsed RPC receipt wait. The pen
    // still proves acceptance, so this cannot be treated as missing evidence.
    let path = lab
        .root
        .path()
        .join("pens/test/raw/.after")
        .join(format!("{id}.json"));
    let lock = fs::OpenOptions::new()
        .write(true)
        .open(path.with_extension("lock"))
        .unwrap();
    loop {
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut record: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    record["send_deadline"] = json!(0);
    fs::write(&path, record.to_string()).unwrap();
    drop(lock);
    lab.json(&["upgrade", "test/raw", "--exe", lab.new.to_str().unwrap()]);
    assert_eq!(lab.record(id)["phase"], "sending");
    assert_eq!(line(&mut blocker)["ok"], true);
    loop {
        let r = lab.record(id);
        if r["phase"] == "sent" {
            break;
        }
        assert!(Instant::now() < until, "{r}");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        String::from_utf8_lossy(&lab.run(&["read", "test/raw"]).stdout)
            .matches("accepted reminder")
            .count(),
        2
    );
}
