//! canter's public lifecycle, with isolated homes and a synthetic terminal/server.
mod common;
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Output},
    time::{Duration, Instant},
};

const NAME: &str = "ranch/test-r56b-fake";
struct Lab {
    root: tempfile::TempDir,
}
impl Lab {
    fn new() -> Self {
        let lab = Self {
            root: tempfile::tempdir_in("/tmp").unwrap(),
        };
        common::copy_exe(env!("CARGO_BIN_EXE_corral"), lab.path("corral"));
        fs::create_dir(lab.path("work")).unwrap();
        fs::create_dir(lab.path("canter-home")).unwrap();
        lab.script("if [ \"$1\" = --version ]; then echo 'canter test'; exit; fi\nprintf '%s\\n' \"$@\" > \"$HOME/args\"\npwd > \"$HOME/cwd\"\nprintf '%s' \"$EXAMPLE\" > \"$HOME/env\"\nexec /bin/cat");
        lab
    }
    fn path(&self, p: &str) -> PathBuf {
        self.root.path().join(p)
    }
    fn script(&self, body: &str) {
        fs::write(self.path("canter"), format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(self.path("canter"), fs::Permissions::from_mode(0o700)).unwrap();
    }
    fn out(&self, args: &[&str]) -> Output {
        Command::new(self.path("corral"))
            .args(args)
            .env("HOME", self.root.path())
            .env("CORRAL_HOME", self.path("pens"))
            .env("CANTER_HOME", self.path("canter-home"))
            .env("CAIRN_DISABLE", "1")
            .env("SHELL", "/bin/sh")
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.root.path().display()),
            )
            .env_remove("CORRAL_NAME")
            .env_remove("CORRAL_INSTANCE")
            .env_remove("CODEX_SANDBOX")
            .output()
            .unwrap()
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
    fn status(&self) -> Value {
        self.json(&["status", NAME])
    }
    fn start(&self, tail: &[&str]) -> Value {
        let cwd = self.path("work");
        let program = self.path("canter");
        let mut args = vec![
            "start",
            NAME,
            "--cwd",
            cwd.to_str().unwrap(),
            "--label",
            "role=test",
            "--env",
            "EXAMPLE=hello",
        ];
        args.extend(tail.iter().map(|a| {
            if *a == "canter" {
                program.to_str().unwrap()
            } else {
                a
            }
        }));
        self.json(&args)
    }
}
impl Drop for Lab {
    fn drop(&mut self) {
        let _ = self.out(&["stop", NAME, "--timeout", "25"]);
    }
}
fn eventually(mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !check() {
        assert!(Instant::now() < deadline, "condition not reached");
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn launch_tracks_session_and_preserves_prompt_cwd_env_labels() {
    let lab = Lab::new();
    let ready = lab.start(&[
        "--prompt",
        "-first\nsecond",
        "--",
        "canter",
        "--yolo",
        "--close-on-exit",
    ]);
    eventually(|| fs::read_to_string(lab.path("env")).is_ok_and(|s| s == "hello"));
    let args = fs::read_to_string(lab.path("args")).unwrap();
    assert_eq!(
        args,
        format!(
            "--session-file\n{}\n--yolo\n--close-on-exit\n--\n-first\nsecond\n",
            lab.path("pens")
                .join(NAME)
                .join("canter-session.json")
                .display()
        )
    );
    assert_eq!(ready["kind"], "canter");
    assert_eq!(fs::read_to_string(lab.path("env")).unwrap(), "hello");
    assert_eq!(
        fs::canonicalize(fs::read_to_string(lab.path("cwd")).unwrap().trim()).unwrap(),
        fs::canonicalize(lab.path("work")).unwrap()
    );
    assert_eq!(lab.status()["labels"], json!({"role":"test"}));
}

#[test]
fn unsupported_operations_and_owned_session_file_are_rejected() {
    let lab = Lab::new();
    let program = lab.path("canter");
    for flag in ["--session-file", "--session-file=/tmp/elsewhere"] {
        let out = lab.out(&["start", NAME, "--", program.to_str().unwrap(), flag]);
        assert_eq!(out.status.code(), Some(1));
        assert_eq!(
            serde_json::from_slice::<Value>(&out.stdout).unwrap()["error"],
            "usage"
        );
    }
    let out = lab.out(&[
        "start",
        NAME,
        "--sandbox",
        "no-such-profile",
        "--",
        program.to_str().unwrap(),
    ]);
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["error"],
        "unsupported_kind"
    );
    assert!(!lab.path("pens").join(NAME).join("sock").exists());
    lab.start(&["--", "canter"]);
    for op in ["pause", "resume"] {
        let out = lab.out(&[op, NAME]);
        assert_eq!(out.status.code(), Some(1));
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["error"], "unsupported_kind");
        assert_eq!(v["name"], NAME);
    }
    assert_eq!(lab.status()["unsupported"], json!(["pause", "sandbox"]));
    assert_eq!(
        lab.json(&["ls"])["agents"][0]["unsupported"],
        json!(["pause", "sandbox"])
    );
    let integrations = lab.json(&["integration", "status"]);
    let item = integrations["agents"].as_array().unwrap().last().unwrap();
    assert_eq!(item["kind"], "canter");
    assert_eq!(item["version"], "canter test");
    assert_eq!(item["hooks"], "launch");
    assert_eq!(item["skills"][0]["target"], "codex");
    assert_eq!(item["unsupported"], json!(["pause", "sandbox"]));
    assert!(item.get("install").is_none());
}

fn http(socket: &std::path::Path, method: &str, path: &str, body: Value) -> Value {
    use std::io::{Read, Write};
    let mut stream = std::os::unix::net::UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let body = body.to_string();
    write!(stream, "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
    let mut bytes = String::new();
    stream.read_to_string(&mut bytes).unwrap();
    let (headers, body) = bytes.split_once("\r\n\r\n").unwrap();
    assert!(headers.starts_with("HTTP/1.1 200"), "{headers}: {body}");
    serde_json::from_str(body).unwrap()
}

struct ChildGuard(std::process::Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct StopBeforeServer<'a>(&'a Lab);
impl Drop for StopBeforeServer<'_> {
    fn drop(&mut self) {
        let _ = self.0.out(&["stop", NAME, "--timeout", "25"]);
    }
}

#[test]
#[ignore = "requires installed canter; isolated homes and a local fake model, no real model calls"]
fn real_canter_lifecycle() {
    let canter = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|dir| dir.join("canter"))
        .find(|p| p.is_file());
    let Some(canter) = canter else {
        eprintln!("skipping: canter is not on PATH");
        return;
    };
    let lab = Lab::new();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let config = json!({"base_url":format!("http://{}", listener.local_addr().unwrap()),"model":"fake","models":["fake"],"context_window":131072,"api_key":"fake-test-key","prompt_suggestions":false});
    fs::write(lab.path("canter-home/config.json"), config.to_string()).unwrap();
    fs::set_permissions(
        lab.path("canter-home/config.json"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let _model = ModelServer::start(listener);
    let log = fs::File::create(lab.path("canter.log")).unwrap();
    let _server = ChildGuard(
        Command::new(&canter)
            .arg("serve")
            .env("HOME", lab.root.path())
            .env("CANTER_HOME", lab.path("canter-home"))
            .env("CORRAL_HOME", lab.path("pens"))
            .env("CAIRN_DISABLE", "1")
            .stdin(std::process::Stdio::null())
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .spawn()
            .unwrap(),
    );
    let _stop = StopBeforeServer(&lab);
    eventually(|| lab.path("canter-home/server.json").exists());
    let registration: Value =
        serde_json::from_slice(&fs::read(lab.path("canter-home/server.json")).unwrap()).unwrap();
    let socket = PathBuf::from(registration["socketPath"].as_str().unwrap());
    http(
        &socket,
        "POST",
        "/v1/folderTrust",
        json!({"cwd":lab.path("work")}),
    );
    let home = format!("HOME={}", lab.root.path().display());
    let state = format!("CANTER_HOME={}", lab.path("canter-home").display());
    lab.start(&[
        "--env",
        &home,
        "--env",
        &state,
        "--env",
        "CAIRN_DISABLE=1",
        "--prompt",
        "r56b-initial",
        "--",
        canter.to_str().unwrap(),
    ]);
    let published = lab.path("pens").join(NAME).join("canter-session.json");
    eventually(|| published.exists());
    let session: Value = serde_json::from_slice(&fs::read(published).unwrap()).unwrap();
    lab.wait_state("idle");
    let snapshot = http(
        &socket,
        "GET",
        &format!("/v1/sessions/{}", session["sessionId"].as_str().unwrap()),
        Value::Null,
    );
    assert_eq!(snapshot["session"]["status"], "idle");
    let sent = lab.json(&["send", NAME, "r56b-first\nsecond line"]);
    assert_eq!(sent["confirmed"], true);
    lab.wait_state("idle");
    assert_eq!(lab.json(&["reply", NAME])["text"], "real fake-model answer");
    lab.json(&["send", NAME, "r56b-question"]);
    assert_eq!(
        lab.wait_state("blocked")["questions"]["items"][0]["question"],
        "Which color?"
    );
    eventually(|| {
        String::from_utf8_lossy(&lab.out(&["read", NAME]).stdout).contains("number keys pick")
    });
    std::thread::sleep(Duration::from_millis(600));
    lab.json(&["keys", NAME, "enter"]);
    lab.wait_state("idle");
    lab.json(&["send", NAME, "r56b-command"]);
    assert_eq!(
        lab.wait_state("blocked")["command"]["text"],
        "mkdir -p r56b-output && touch r56b-output/probe"
    );
    lab.json(&["keys", NAME, "esc"]);
    assert_eq!(lab.wait_state("idle")["interrupted"], true);
    common::copy_exe(env!("CARGO_BIN_EXE_corral"), lab.path("new-corral"));
    let upgraded = lab.json(&[
        "upgrade",
        NAME,
        "--exe",
        lab.path("new-corral").to_str().unwrap(),
    ]);
    assert_eq!(upgraded["result"], "complete");
    lab.json(&["send", NAME, "r56b-interrupt"]);
    lab.wait_state("working");
    lab.json(&["keys", NAME, "esc"]);
    assert_eq!(lab.wait_state("idle")["interrupted"], true);
    lab.json(&["send", NAME, "r56b-interrupt again"]);
    lab.wait_state("working");
    let stopped = lab.json(&["stop", NAME]);
    assert!(stopped.get("remaining_sessions").is_none(), "{stopped}");
    assert_eq!(
        http(
            &socket,
            "GET",
            &format!("/v1/sessions/{}", session["sessionId"].as_str().unwrap()),
            Value::Null
        )["session"]["status"],
        "closed"
    );
}

use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
#[derive(Default)]
struct FakeState {
    sessions: BTreeMap<String, Value>,
    events: BTreeMap<String, Vec<Value>>,
    requests: Vec<String>,
}
struct FakeServer {
    state: Arc<Mutex<FakeState>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl FakeServer {
    fn new(lab: &Lab, socket_name: &str) -> Self {
        let socket = lab.path(socket_name);
        let listener = UnixListener::bind(&socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        fs::write(lab.path("canter-home/server.json"), json!({"socketPath":socket,"protocolVersion":1,"version":"fake","instance":socket_name}).to_string()).unwrap();
        let state = Arc::new(Mutex::new(FakeState::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let shared = state.clone();
        let done = stop.clone();
        let thread = std::thread::spawn(move || {
            let mut workers = Vec::new();
            while !done.load(Ordering::SeqCst) {
                if let Ok((stream, _)) = listener.accept() {
                    let state = shared.clone();
                    let stop = done.clone();
                    workers.push(std::thread::spawn(move || {
                        fake_connection(stream, state, stop)
                    }));
                } else {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
            for worker in workers {
                worker.join().unwrap();
            }
        });
        Self {
            state,
            stop,
            thread: Some(thread),
        }
    }
    fn session(&self, id: &str, created: f64) {
        self.state.lock().unwrap().sessions.insert(id.into(), json!({"id":id,"status":"idle","createdAt":created*1000.,"lastEventSeq":0,"pendingRequests":[],"turns":[]}));
    }
    fn emit(&self, id: &str, kind: &str, extra: Value) {
        let mut state = self.state.lock().unwrap();
        let seq = state.events.get(id).map_or(1, |v| v.len() + 1);
        let mut event = json!({"type":kind,"seq":seq,"sessionId":id,"timestamp":millis()});
        event
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        state.sessions.get_mut(id).unwrap()["lastEventSeq"] = json!(seq);
        if kind == "session.status" {
            state.sessions.get_mut(id).unwrap()["status"] = event["status"].clone();
        }
        state.events.entry(id.into()).or_default().push(event);
    }
    fn close_all(&self) {
        let ids: Vec<_> = self
            .state
            .lock()
            .unwrap()
            .sessions
            .keys()
            .cloned()
            .collect();
        for id in ids {
            self.emit(
                &id,
                "session.status",
                json!({"status":"closed","closeReason":"clientRequest"}),
            );
        }
    }
}
impl Drop for FakeServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
    }
}
fn millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
fn fake_connection(mut stream: UnixStream, state: Arc<Mutex<FakeState>>, stop: Arc<AtomicBool>) {
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let path = line.split_whitespace().nth(1).unwrap_or("").to_owned();
    state.lock().unwrap().requests.push(path.clone());
    if path.starts_with("/v1/events?") {
        assert!(
            path.contains("observe=true"),
            "observer must never own a canter lease"
        );
        let parts: BTreeMap<_, _> = path
            .split_once('?')
            .unwrap()
            .1
            .split('&')
            .filter_map(|s| s.split_once('='))
            .collect();
        let id = parts["session"];
        let mut after = parts["after"].parse::<u64>().unwrap();
        if stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n").is_err() { return; }
        while !stop.load(Ordering::SeqCst) {
            let events = state
                .lock()
                .unwrap()
                .events
                .get(id)
                .cloned()
                .unwrap_or_default();
            let mut body = String::from(": heartbeat\n\n");
            for event in events.iter().filter(|e| e["seq"].as_u64().unwrap() > after) {
                body.push_str(&format!(
                    "id: {}\nevent: message\ndata: {event}\n\n",
                    event["seq"]
                ));
            }
            if let Some(last) = events.last() {
                after = last["seq"].as_u64().unwrap();
            }
            // Split HTTP chunks independently of SSE frames.
            for bytes in body.as_bytes().chunks(23) {
                if write!(stream, "{:x}\r\n", bytes.len())
                    .and_then(|_| stream.write_all(bytes))
                    .and_then(|_| stream.write_all(b"\r\n"))
                    .is_err()
                {
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    } else {
        let response = if path == "/v1/info" {
            json!({"protocolVersion":1,"features":["managedClients"]})
        } else if let Some(id) = path.strip_prefix("/v1/sessions/") {
            json!({"session":state.lock().unwrap().sessions.get(id)})
        } else {
            panic!("unexpected request {path}")
        };
        let body = response.to_string();
        if path == "/v1/info" {
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{body}\r\n0\r\n\r\n",
                body.len()
            );
        } else {
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    }
}
impl Lab {
    fn publish(&self, id: &str) {
        let path = self.path("pens").join(NAME).join("canter-session.json");
        let temp = path.with_extension("tmp");
        fs::write(
            &temp,
            json!({"sessionId":id,"stateDir":self.path("canter-home"),"cwd":self.path("work")})
                .to_string(),
        )
        .unwrap();
        fs::rename(temp, path).unwrap();
    }
    fn wait_state(&self, expected: &str) -> Value {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let last = self.status();
            if last["state"] == expected {
                return last;
            }
            assert!(
                Instant::now() < deadline,
                "expected {expected}, got {last}; events: {:?}; log: {}; read: {}",
                self.records(),
                fs::read_to_string(self.path("pens").join(NAME).join("pen.log"))
                    .unwrap_or_default(),
                String::from_utf8_lossy(&self.out(&["read", NAME]).stdout)
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

#[test]
fn late_session_file_streams_turns_dialogs_and_interruption_through_public_commands() {
    let lab = Lab::new();
    let server = FakeServer::new(&lab, "server.sock");
    lab.start(&["--", "canter"]);
    assert_eq!(lab.status()["state"], "starting");
    server.session("s1", millis() as f64 / 1000. + 0.001);
    lab.publish("s1");
    lab.wait_state("idle");
    server.emit(
        "s1",
        "turn.started",
        json!({"turn":{"id":"t1","input":[{"type":"text","text":"hello"}]}}),
    );
    lab.wait_state("working");
    server.emit("s1", "request.opened", json!({"request":{"id":"r","createdAt":millis(),"kind":{"type":"approval","tool":"shell","arguments":{"command":"echo hello"}}}}));
    assert_eq!(lab.wait_state("blocked")["command"]["text"], "echo hello");
    server.emit("s1", "request.resolved", json!({"requestId":"r"}));
    lab.wait_state("working");
    server.emit(
        "s1",
        "item.completed",
        json!({"item":{"type":"agentMessage","text":"finished"}}),
    );
    server.emit(
        "s1",
        "turn.completed",
        json!({"turn":{"status":"completed"}}),
    );
    assert_eq!(lab.status()["state"], "working");
    server.emit("s1", "session.status", json!({"status":"idle"}));
    lab.wait_state("idle");
    assert_eq!(lab.json(&["reply", NAME])["text"], "finished");
    assert_eq!(
        lab.json(&["wait", NAME, "--timeout", "1"])["result"],
        "idle"
    );
    server.emit("s1", "turn.started", json!({"turn":{"id":"t2","input":[]}}));
    lab.wait_state("working");
    server.emit(
        "s1",
        "turn.completed",
        json!({"turn":{"status":"interrupted"}}),
    );
    server.emit("s1", "session.status", json!({"status":"idle"}));
    assert_eq!(lab.wait_state("idle")["interrupted"], true);
    assert_eq!(
        lab.json(&["wait", NAME, "--timeout", "1"])["result"],
        "interrupted"
    );
    let reply: Value = serde_json::from_slice(&lab.out(&["reply", NAME]).stdout).unwrap();
    assert_eq!(reply["error"], "no_reply");
    assert_eq!(reply["interrupted"], true);
    server.close_all();
    assert!(
        lab.json(&["stop", NAME])
            .get("remaining_sessions")
            .is_none()
    );
}

impl Lab {
    fn request(&self, value: Value) -> Value {
        let mut socket = UnixStream::connect(self.path("pens").join(NAME).join("sock")).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(6)))
            .unwrap();
        writeln!(socket, "{value}").unwrap();
        let mut line = String::new();
        BufReader::new(socket).read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }
    fn records(&self) -> Vec<Value> {
        fs::read_to_string(self.path("pens").join(NAME).join("events"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
}
fn turn(server: &FakeServer, id: &str, text: &str) {
    server.emit(
        id,
        "turn.started",
        json!({"turn":{"input":[{"type":"text","text":text}]}}),
    );
    server.emit(
        id,
        "item.completed",
        json!({"item":{"type":"agentMessage","text":text}}),
    );
    server.emit(id, "turn.completed", json!({"turn":{"status":"completed"}}));
    server.emit(id, "session.status", json!({"status":"idle"}));
}
#[test]
fn session_switch_resume_socket_replacement_and_pen_upgrade_preserve_event_cursor() {
    let lab = Lab::new();
    let server = FakeServer::new(&lab, "first.sock");
    lab.start(&["--", "canter"]);
    server.session("first", millis() as f64 / 1000. + 0.001);
    lab.publish("first");
    lab.wait_state("idle");
    turn(&server, "first", "first reply");
    eventually(|| {
        lab.records()
            .iter()
            .any(|e| e["last_assistant_message"] == "first reply")
    });
    let before = lab.records();
    common::copy_exe(env!("CARGO_BIN_EXE_corral"), lab.path("new-corral"));
    let request = lab.request(json!({"op":"upgrade","exe":lab.path("new-corral")}));
    assert_eq!(request["result"], "accepted");
    turn(&server, "first", "during upgrade");
    eventually(|| lab.status()["upgrade"]["result"] == "complete");
    eventually(|| {
        lab.records()
            .iter()
            .any(|e| e["last_assistant_message"] == "during upgrade")
    });
    assert_eq!(&lab.records()[..before.len()], &before);
    assert_eq!(
        lab.records()
            .iter()
            .filter(|e| e["ev"] == "SessionStart")
            .count(),
        1
    );
    assert_eq!(
        lab.records()
            .iter()
            .filter(|e| e["last_assistant_message"] == "first reply")
            .count(),
        1
    );
    assert!(
        server
            .state
            .lock()
            .unwrap()
            .requests
            .iter()
            .any(|r| r.ends_with("after=4"))
    );
    server.emit(
        "first",
        "session.status",
        json!({"status":"closed","closeReason":"serverShutdown"}),
    );
    let replacement = FakeServer::new(&lab, "second.sock");
    {
        let old = server.state.lock().unwrap();
        let mut new = replacement.state.lock().unwrap();
        new.sessions = old.sessions.clone();
        new.events = old.events.clone();
    }
    drop(server);
    replacement.emit("first", "session.status", json!({"status":"idle"}));
    turn(&replacement, "first", "after replace");
    eventually(|| {
        lab.records()
            .iter()
            .any(|e| e["last_assistant_message"] == "after replace")
    });
    assert!(!lab.records().iter().any(|e| e["ev"] == "SessionEnd"));
    replacement.session("new", millis() as f64 / 1000. + 0.001);
    lab.publish("new");
    eventually(|| {
        lab.records()
            .iter()
            .any(|e| e["session_id"] == "new" && e["ev"] == "SessionStart")
    });
    replacement.session("old", 1.);
    turn(&replacement, "old", "old reply must not be published");
    lab.publish("old");
    eventually(|| {
        lab.records()
            .iter()
            .any(|e| e["session_id"] == "old" && e["ev"] == "SessionStart")
    });
    assert!(
        !lab.records()
            .iter()
            .any(|e| e["last_assistant_message"] == "old reply must not be published")
    );
    eventually(|| {
        replacement
            .state
            .lock()
            .unwrap()
            .requests
            .iter()
            .any(|r| r == "/v1/events?session=old&observe=true&after=4")
    });
    replacement.close_all();
    assert!(
        lab.json(&["stop", NAME])
            .get("remaining_sessions")
            .is_none()
    );
}

#[test]
fn older_upgrade_targets_cannot_silently_remove_the_watcher() {
    let lab = Lab::new();
    lab.start(&["--", "canter"]);
    let target = lab.path("legacy-corral");
    fs::write(
        &target,
        "#!/bin/sh\necho '{\"ok\":true,\"schema\":1,\"pause\":1}'\n",
    )
    .unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
    let reply = lab.request(json!({"op":"upgrade","exe":target}));
    assert_eq!(reply["error"], "incompatible");
    for op in ["pause", "resume"] {
        assert_eq!(lab.request(json!({"op":op}))["error"], "unsupported_kind");
    }
}

#[test]
fn stop_reports_unconfirmed_sessions_but_still_succeeds() {
    let lab = Lab::new();
    let server = FakeServer::new(&lab, "unclosed.sock");
    lab.start(&["--", "canter"]);
    server.session("unclosed", millis() as f64 / 1000. + 0.001);
    lab.publish("unclosed");
    lab.wait_state("idle");
    let started = Instant::now();
    let stopped = lab.json(&["stop", NAME]);
    assert!(started.elapsed() >= Duration::from_secs(12));
    assert!(started.elapsed() < Duration::from_secs(17));
    assert_eq!(stopped["remaining_sessions"], json!(["unclosed"]));
    assert_eq!(stopped["ok"], true);
    let exit: Value =
        serde_json::from_slice(&fs::read(lab.path("pens").join(NAME).join("exit.json")).unwrap())
            .unwrap();
    assert_eq!(exit["remaining_sessions"], stopped["remaining_sessions"]);
}

struct ModelServer {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl ModelServer {
    fn start(listener: std::net::TcpListener) -> Self {
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let thread = std::thread::spawn(move || {
            let mut workers = Vec::new();
            while !done.load(Ordering::SeqCst) {
                if let Ok((mut stream, _)) = listener.accept() {
                    let done = done.clone();
                    workers.push(std::thread::spawn(move || {
                        use std::io::Read;
                        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                        let mut reader = BufReader::new(stream.try_clone().unwrap());
                        let mut length = 0;
                        loop {
                            let mut line = String::new();
                            if reader.read_line(&mut line).unwrap_or(0) == 0 { return; }
                            if line == "\r\n" { break; }
                            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                                length = value.trim().parse::<usize>().unwrap();
                            }
                        }
                        let mut body = vec![0;length]; reader.read_exact(&mut body).unwrap();
                        let request: Value = serde_json::from_slice(&body).unwrap();
                        let last = request["messages"].as_array().unwrap().last().unwrap();
                        let content = last["content"].as_str().unwrap_or("");
                        let is_tool = last["role"] == "tool";
                        let tool = if !is_tool && content.contains("r56b-command") {
                            Some(("shell", json!({"command":"mkdir -p r56b-output && touch r56b-output/probe"})))
                        } else if !is_tool && content.contains("r56b-question") {
                            Some(("ask_user", json!({"question":"Which color?","options":[{"label":"Red","description":"warm"},{"label":"Blue","description":"cool"}]})))
                        } else { None };
                        let delta = if let Some((name, arguments)) = &tool { json!({"tool_calls":[{"index":0,"id":"call-test","type":"function","function":{"name":name,"arguments":arguments.to_string()}}]}) }
                            else { json!({"content":"real fake-model answer"}) };
                        if stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n").is_err() { return; }
                        if writeln!(stream, "data: {}\n", json!({"choices":[{"index":0,"delta":delta,"finish_reason":null}]})).is_err() { return; }
                        if content.contains("r56b-interrupt") && !is_tool {
                            while !done.load(Ordering::SeqCst) { std::thread::sleep(Duration::from_millis(50)); }
                            return;
                        }
                        let _ = write!(stream,"data: {}\n\ndata: [DONE]\n\n",json!({"choices":[{"index":0,"delta":{},"finish_reason":if tool.is_some(){"tool_calls"}else{"stop"}}]}));
                    }));
                } else {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
            for worker in workers {
                worker.join().unwrap();
            }
        });
        Self {
            stop,
            thread: Some(thread),
        }
    }
}
impl Drop for ModelServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
    }
}

#[test]
fn inherited_sandbox_is_rejected_before_calling_fence() {
    let lab = Lab::new();
    lab.start(&["--", "canter"]);
    let status = lab.status();
    fs::write(
        lab.path("pens").join(NAME).join("child-sandbox.json"),
        json!({"instance":status["instance"],"child_sandbox":{"profile":"missing-profile"}})
            .to_string(),
    )
    .unwrap();
    let out = Command::new(lab.path("corral"))
        .args([
            "start",
            "ranch/test-r56b-child",
            "--",
            lab.path("canter").to_str().unwrap(),
        ])
        .env("HOME", lab.root.path())
        .env("CORRAL_HOME", lab.path("pens"))
        .env("CANTER_HOME", lab.path("canter-home"))
        .env("SHELL", "/bin/sh")
        .env("CORRAL_NAME", NAME)
        .env("CORRAL_INSTANCE", status["instance"].as_str().unwrap())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["error"],
        "unsupported_kind"
    );
    assert!(!lab.path("pens/ranch/test-r56b-child/sock").exists());
}

#[test]
fn old_pen_without_watcher_information_is_unknown() {
    let lab = Lab::new();
    // An old pen's public protocol does not have watcher status, even for a known kind.
    let dir = lab.path("pens").join(NAME);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("meta.json"),
        json!({"kind":"canter","cwd":lab.path("work"),"instance":"old","proto":1}).to_string(),
    )
    .unwrap();
    let listener = UnixListener::bind(dir.join("sock")).unwrap();
    let thread = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut line = String::new();
        BufReader::new(socket.try_clone().unwrap())
            .read_line(&mut line)
            .unwrap();
        writeln!(
            socket,
            "{}",
            json!({"ok":true,"proto":1,"instance":"old","started":1,"title":"canter"})
        )
        .unwrap();
    });
    assert_eq!(lab.status()["state"], "unknown");
    thread.join().unwrap();
    fs::remove_file(dir.join("sock")).unwrap();
}
