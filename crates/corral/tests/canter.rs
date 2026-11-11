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
    lab.json(&["send", NAME, "r56b-retract"]);
    lab.wait_state("working");
    lab.json(&["keys", NAME, "esc"]);
    let interrupted = lab.wait_state("idle");
    assert_eq!(interrupted["interrupted"], true);
    assert_eq!(interrupted["interaction"]["running"], false);
    let reply = lab.out(&["reply", NAME]);
    assert_eq!(
        serde_json::from_slice::<Value>(&reply.stdout).unwrap()["error"],
        "no_reply"
    );
    assert!(!reply.status.success());
    // Esc retracts the input back into canter's composer; clear it before the next prompt.
    lab.json(&["keys", NAME, "text:\u{15}"]);
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
    delayed_heartbeat: bool,
    input_reply: Option<(u16, Value)>,
    disconnect_input: bool,
    hold_input: bool,
    inputs: Vec<Value>,
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
    if path.ends_with("/input") || path.contains("/requests/") {
        use std::io::Read;
        assert_eq!(line.split_whitespace().next(), Some("POST"));
        let mut length = 0;
        loop {
            line.clear();
            assert!(reader.read_line(&mut line).unwrap() > 0);
            if line == "\r\n" {
                break;
            }
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                length = value.trim().parse::<usize>().unwrap();
            }
        }
        let mut bytes = vec![0; length];
        reader.read_exact(&mut bytes).unwrap();
        let reply = {
            let mut state = state.lock().unwrap();
            state.inputs.push(serde_json::from_slice(&bytes).unwrap());
            if state.disconnect_input {
                return;
            }
            state.input_reply.clone()
        };
        while state.lock().unwrap().hold_input && !stop.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(10));
        }
        let Some((status, body)) = reply else { return };
        let body = body.to_string();
        let _ = write!(
            stream,
            "HTTP/1.1 {status} Reply\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
    } else if path.starts_with("/v1/events?") {
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
        let delayed = state.lock().unwrap().delayed_heartbeat;
        let heartbeat_interval = if delayed {
            Duration::from_secs(15)
        } else {
            Duration::from_millis(50)
        };
        let mut heartbeat_at = Instant::now()
            + if delayed {
                heartbeat_interval
            } else {
                Duration::ZERO
            };
        while !stop.load(Ordering::SeqCst) {
            let events = state
                .lock()
                .unwrap()
                .events
                .get(id)
                .cloned()
                .unwrap_or_default();
            let mut body = String::new();
            if Instant::now() >= heartbeat_at {
                body.push_str(": heartbeat\n\n");
                heartbeat_at = Instant::now() + heartbeat_interval;
            }
            for event in events.iter().filter(|e| e["seq"].as_u64().unwrap() > after) {
                let data = if event["malformed"] == true {
                    "{".into()
                } else {
                    event.to_string()
                };
                body.push_str(&format!(
                    "id: {}\nevent: message\ndata: {data}\n\n",
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
    fn working(&self, server: &FakeServer) {
        self.start(&["--", "canter"]);
        server.session("s1", millis() as f64 / 1000. + 0.001);
        self.publish("s1");
        self.wait_state("idle");
        server.emit(
            "s1",
            "turn.started",
            json!({"turn":{"id":"t1","input":[{"type":"text","text":"base task"}]}}),
        );
        self.wait_state("working");
    }
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
fn answer_status_panic_withholds_the_request_and_keeps_the_pen_alive() {
    let lab = Lab::new();
    let server = FakeServer::new(&lab, "answer-panic.sock");
    lab.working(&server);
    server.emit("s1", "request.opened", json!({"request":{"id":"r1","createdAt":1,"kind":{
        "type":"userInput","question":"Color?","options":[{"label":"Blue","description":"cool"}]}}}));
    let status = lab.wait_state("blocked");
    let before = lab.request(json!({"op":"status"}));
    let events = lab.path("pens").join(NAME).join("events");
    let path = events.with_file_name("cursor");
    let mut cursor: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    cursor["raw"] = json!({"main":{"inputs":null}});
    let temp = events.with_file_name("panic-cursor.tmp");
    fs::write(&temp, serde_json::to_vec(&cursor).unwrap()).unwrap();
    fs::rename(temp, path).unwrap();
    let mut file = fs::OpenOptions::new().append(true).open(&events).unwrap();
    writeln!(
        file,
        "{}",
        json!({"v":2,"inst":cursor["inst"],"session_id":"main","adapter":"omp","has_ui":true,
        "ev":"input","t":1.0,"event":{"text":"synthetic panic","streamingBehavior":"steer"}})
    )
    .unwrap();
    let receipt = lab.request(json!({"op":"answer","instance":status["instance"],"request_id":"r1","answer":{"type":"option","text":"Blue"}}));
    assert_eq!(receipt["error"], "state_check_failed", "{receipt}");
    assert_eq!(receipt["written"], false);
    assert_eq!(receipt["submitted"], false);
    assert_eq!(receipt["confirmed"], false);
    let after = lab.request(json!({"op":"status"}));
    assert_eq!(after["instance"], before["instance"]);
    assert_eq!(after["agent_pid"], before["agent_pid"]);
    assert!(server.state.lock().unwrap().inputs.is_empty());
    fs::remove_file(events.with_file_name("cursor")).unwrap();
    server.close_all();
    lab.json(&["stop", NAME]);
}

#[test]
fn answer_option_uses_the_request_api_without_terminal_input() {
    let lab = Lab::new();
    let server = FakeServer::new(&lab, "answer.sock");
    lab.working(&server);
    server.emit("s1", "request.opened", json!({"request":{"id":"r/1","createdAt":1,"kind":{
        "type":"userInput","question":"Color?","options":[{"label":"Blue","description":"cool"}]}}}));
    let status = lab.wait_state("blocked");
    server.state.lock().unwrap().input_reply =
        Some((200, json!({"eventSeq":3,"alreadyResolved":false})));
    let before = lab.out(&["read", NAME]).stdout;
    let out = lab.out(&[
        "answer",
        NAME,
        "--instance",
        status["instance"].as_str().unwrap(),
        "--request",
        "r/1",
        "--option",
        "Blue",
    ]);
    let after = lab.out(&["read", NAME]).stdout;
    server.close_all();
    lab.json(&["stop", NAME]);
    let receipt: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(0), "{receipt}");
    assert_eq!(receipt["already_resolved"], false);
    assert_eq!(receipt["confirmed"], true);
    assert_eq!(before, after);
    let state = server.state.lock().unwrap();
    assert_eq!(
        state.inputs,
        vec![json!({"answer":{"type":"option","label":"Blue"},"clientName":"corral"})]
    );
    assert!(
        state
            .requests
            .iter()
            .any(|p| p == "/v1/sessions/s1/requests/r%2F1")
    );
}

#[test]
fn answer_text_allow_once_and_plan_send_only_the_requested_safe_decision() {
    for (kind, flags, expected) in [
        (
            json!({"type":"userInput","question":"Name?","options":[]}),
            vec!["--text", "A short answer"],
            json!({"answer":{"type":"text","text":"A short answer"},"clientName":"corral"}),
        ),
        (
            json!({"type":"approval","tool":"shell","arguments":{"command":"pwd"},"availableDecisions":["accept","decline"]}),
            vec!["--allow-once"],
            json!({"decision":"accept","clientName":"corral"}),
        ),
        (
            json!({"type":"approval","tool":"edit","arguments":{"path":"notes.txt"},"availableDecisions":["accept"]}),
            vec!["--allow-once"],
            json!({"decision":"accept","clientName":"corral"}),
        ),
        (
            json!({"type":"planApproval","plan":"Read notes and summarize."}),
            vec!["--approve-plan"],
            json!({"plan":{"decision":"implement","approvalPolicy":"alwaysAsk"},"clientName":"corral"}),
        ),
    ] {
        let lab = Lab::new();
        let server = FakeServer::new(&lab, "answer.sock");
        lab.working(&server);
        server.emit(
            "s1",
            "request.opened",
            json!({"request":{"id":"r1","createdAt":1,"kind":kind}}),
        );
        let status = lab.wait_state("blocked");
        server.state.lock().unwrap().input_reply =
            Some((200, json!({"eventSeq":3,"alreadyResolved":false})));
        let mut args = vec![
            "answer",
            NAME,
            "--instance",
            status["instance"].as_str().unwrap(),
            "--request",
            "r1",
        ];
        args.extend(flags);
        let out = lab.out(&args);
        server.close_all();
        lab.json(&["stop", NAME]);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
        assert_eq!(server.state.lock().unwrap().inputs, [expected]);
    }
}

#[test]
fn answer_receipts_distinguish_already_resolved_rejection_and_uncertainty() {
    for (case, reply, code) in [
        (
            "resolved",
            Some((200, json!({"eventSeq":4,"alreadyResolved":true}))),
            0,
        ),
        (
            "rejected",
            Some((
                400,
                json!({"code":"invalid_request","message":"invalid answer"}),
            )),
            7,
        ),
        (
            "internal",
            Some((500, json!({"code":"internal","message":"failure"}))),
            3,
        ),
        ("invalid", Some((200, json!({"eventSeq":4}))), 3),
        ("disconnect", None, 3),
        ("timeout", None, 3),
    ] {
        let lab = Lab::new();
        let server = FakeServer::new(&lab, "answer.sock");
        lab.working(&server);
        server.emit("s1", "request.opened", json!({"request":{"id":"r1","createdAt":1,"kind":{"type":"userInput","question":"Name?","options":[]}}}));
        let status = lab.wait_state("blocked");
        {
            let mut state = server.state.lock().unwrap();
            state.input_reply = reply;
            state.hold_input = case == "timeout";
        }
        let out = std::thread::scope(|scope| {
            let request = scope.spawn(|| {
                lab.out(&[
                    "answer",
                    NAME,
                    "--instance",
                    status["instance"].as_str().unwrap(),
                    "--request",
                    "r1",
                    "--text",
                    "hello",
                ])
            });
            eventually(|| !server.state.lock().unwrap().inputs.is_empty());
            if case == "timeout" {
                assert_eq!(lab.status()["state"], "blocked");
                assert_eq!(
                    lab.request(json!({"op":"upgrade","exe":lab.path("unused")}))["error"],
                    "upgrade_busy"
                );
                assert!(!request.is_finished());
            }
            request.join().unwrap()
        });
        server.state.lock().unwrap().hold_input = false;
        server.close_all();
        lab.json(&["stop", NAME]);
        let receipt: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(out.status.code(), Some(code), "{case}: {receipt}");
        assert_eq!(
            server.state.lock().unwrap().inputs.len(),
            1,
            "no automatic retry"
        );
        if code == 0 {
            assert_eq!(receipt["already_resolved"], true);
        } else {
            assert_eq!(
                receipt["error"],
                if code == 7 {
                    "answer_failed"
                } else {
                    "not_delivered"
                }
            );
            assert_eq!(
                receipt["written"],
                if code == 7 { json!(false) } else { Value::Null }
            );
            assert_eq!(receipt["confirmed"], false);
        }
    }
}

#[test]
fn answer_rechecks_front_instance_session_and_refuses_unsafe_decisions() {
    let lab = Lab::new();
    let server = FakeServer::new(&lab, "answer.sock");
    lab.working(&server);
    server.emit("s1", "request.opened", json!({"request":{"id":"r1","createdAt":1,"kind":{"type":"sandboxApproval","tool":"shell","availableDecisions":["accept"]}}}));
    let status = lab.wait_state("blocked");
    let instance = status["instance"].as_str().unwrap();
    for (instance, id, flag, value, expected) in [
        ("old-instance", "r1", "--allow-once", None, 7),
        (instance, "different", "--allow-once", None, 7),
        (instance, "r1", "--allow-once", None, 7),
        (instance, "r1", "--approve-plan", None, 7),
        (instance, "r1", "--text", Some("first\nsecond"), 7),
    ] {
        let mut args = vec![
            "answer",
            NAME,
            "--instance",
            instance,
            "--request",
            id,
            flag,
        ];
        args.extend(value);
        assert_eq!(lab.out(&args).status.code(), Some(expected));
    }
    assert!(server.state.lock().unwrap().inputs.is_empty());
    let mut attached = UnixStream::connect(lab.path("pens").join(NAME).join("sock")).unwrap();
    attached
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    writeln!(attached, "{}", json!({"op":"attach","rows":40,"cols":120})).unwrap();
    let mut ack = String::new();
    BufReader::new(&attached).read_line(&mut ack).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&ack).unwrap()["ok"], true);
    attached.write_all(b"i\0\0\0\x01h").unwrap();
    eventually(|| lab.status()["last_human_input"].is_number());
    let blocked = lab.out(&[
        "answer",
        NAME,
        "--instance",
        instance,
        "--request",
        "r1",
        "--allow-once",
    ]);
    assert_eq!(blocked.status.code(), Some(8));
    assert_eq!(
        serde_json::from_slice::<Value>(&blocked.stdout).unwrap()["error"],
        "human_active"
    );
    assert!(server.state.lock().unwrap().inputs.is_empty());
    drop(attached);
    server.close_all();
    lab.json(&["stop", NAME]);
}

#[test]
fn answer_status_includes_write_content_and_edit_changes_with_bounded_details() {
    let lab = Lab::new();
    let server = FakeServer::new(&lab, "file-details.sock");
    lab.working(&server);
    for (index, tool, arguments, field, expected) in [
        (
            0,
            "write",
            json!({"path":"notes.txt","content":"hello\n世界"}),
            "content",
            "hello\n世界".to_owned(),
        ),
        (
            1,
            "edit",
            json!({"path":"notes.txt","edits":[{"oldText":"old\n","newText":"new\n"},{"oldText":"delete","newText":""}]}),
            "edits",
            serde_json::to_string_pretty(
                &json!([{"oldText":"old\n","newText":"new\n"},{"oldText":"delete","newText":""}]),
            )
            .unwrap(),
        ),
        (
            2,
            "write",
            json!({"path":"notes.txt","content":"界".repeat(10000)}),
            "content",
            "界".repeat(10000),
        ),
        (
            3,
            "edit",
            json!({"path":"notes.txt","edits":[{"oldText":"old","newText":"界".repeat(10000)}]}),
            "edits",
            String::new(),
        ),
    ] {
        let id = format!("r{index}");
        server.emit(
            "s1",
            "request.opened",
            json!({"request":{"id":id,"createdAt":index,"kind":{
            "type":"approval","tool":tool,"arguments":arguments,"availableDecisions":["accept"]}}}),
        );
        eventually(|| lab.status()["interaction"]["request"]["id"] == id);
        let request = lab.status()["interaction"]["request"].clone();
        assert_eq!(request["file"]["text"], "notes.txt");
        let detail = &request[field];
        assert_eq!(detail["truncated"], index >= 2, "{request}");
        assert!(detail.to_string().len() <= 16 * 1024);
        if index < 2 {
            assert_eq!(detail["text"], expected);
        } else {
            assert!(!detail["text"].as_str().unwrap().is_empty());
        }
        server.emit(
            "s1",
            "request.resolved",
            json!({"requestId":id,"clientName":"test"}),
        );
        lab.wait_state("working");
    }
    server.close_all();
    lab.json(&["stop", NAME]);
}

#[test]
fn answer_status_identifies_the_front_request_and_approval_details() {
    let lab = Lab::new();
    let server = FakeServer::new(&lab, "answer.sock");
    lab.working(&server);
    server.emit(
        "s1",
        "request.opened",
        json!({"request":{"id":"r1","createdAt":1,"kind":{
        "type":"approval","tool":"write","arguments":{"path":"notes.txt","content":"hello"},
        "availableDecisions":["accept","acceptForSession","decline"]}}}),
    );
    let status = lab.wait_state("blocked");
    server.close_all();
    lab.json(&["stop", NAME]);
    assert_eq!(
        status["interaction"]["request"],
        json!({"id":"r1","kind":"approval","tool":"write",
        "file":{"text":"notes.txt","truncated":false},"content":{"text":"hello","truncated":false},"available_decisions":["accept","acceptForSession","decline"]})
    );
}

#[test]
fn steer_working_canter_confirms_the_api_receipt_and_preserves_the_draft() {
    let lab = Lab::new();
    let server = FakeServer::new(&lab, "steer.sock");
    lab.working(&server);
    server.state.lock().unwrap().input_reply = Some((200, json!({"eventSeq":2,"turnId":"t1"})));
    // R57b clients cannot interpret an API receipt; reject before sending on their behalf.
    let legacy = lab.request(json!({"op":"steer","text":"old CLI","request_id":"legacy"}));
    assert_eq!(legacy["error"], "unsupported_steer", "{legacy}");
    assert!(server.state.lock().unwrap().inputs.is_empty());
    lab.json(&["keys", NAME, "text:unsent draft"]);
    let before = lab.out(&["read", NAME]).stdout;
    let text = "改一下当前任务\n保留草稿";
    let out = lab.out(&["send", NAME, text, "--steer", "--request-id", "api-receipt"]);
    let after = lab.out(&["read", NAME]).stdout;
    let status = lab.status();
    server.close_all();
    lab.json(&["stop", NAME]);
    let sent: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(0), "{sent}");
    assert_eq!(sent["confirmed"], true);
    assert_eq!(sent["merged_with_draft"], false);
    assert_eq!(sent["queued_for_next_turn"], false);
    assert_eq!(sent["request_id"], "api-receipt");
    assert_eq!(before, after, "API steering must not write to the terminal");
    let state = server.state.lock().unwrap();
    assert_eq!(
        state.inputs,
        vec![json!({"input":[{"type":"text","text":text}],"mode":"steer","clientName":"corral"})]
    );
    assert!(state.requests.iter().any(|p| p == "/v1/sessions/s1/input"));
    let record = status["recent_sends"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["request_id"] == "api-receipt")
        .unwrap();
    assert_eq!(record["state"], "written");
    assert_eq!(record["confirmed"], true);
    assert_eq!(record["queued_for_next_turn"], false);
}

#[test]
fn steer_working_canter_reports_next_turn_queueing_as_success() {
    for receipt in [json!({"eventSeq":2,"turnId":null}), json!({"eventSeq":2})] {
        let lab = Lab::new();
        let server = FakeServer::new(&lab, "queue.sock");
        lab.working(&server);
        server.state.lock().unwrap().input_reply = Some((200, receipt));
        let out = lab.out(&[
            "send",
            NAME,
            "after compaction",
            "--steer",
            "--timeout",
            "0",
        ]);
        server.close_all();
        lab.json(&["stop", NAME]);
        let sent: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(out.status.code(), Some(0), "{sent}");
        assert_eq!(sent["confirmed"], true);
        assert_eq!(sent["queued_for_next_turn"], true);
        assert_eq!(sent["merged_with_draft"], false);
        assert_eq!(server.state.lock().unwrap().inputs.len(), 1);
    }
}

#[test]
fn steer_working_canter_distinguishes_unsent_or_rejected_input() {
    for failure in ["discovery", "connect", "rejected"] {
        let lab = Lab::new();
        let server = FakeServer::new(&lab, "reject.sock");
        lab.working(&server);
        let registration = fs::read(lab.path("canter-home/server.json")).unwrap();
        match failure {
            "discovery" => fs::write(lab.path("canter-home/server.json"), "{").unwrap(),
            "connect" => fs::write(
                lab.path("canter-home/server.json"),
                json!({"socketPath":lab.path("missing.sock")}).to_string(),
            )
            .unwrap(),
            _ => {
                server.state.lock().unwrap().input_reply = Some((
                    409,
                    json!({"code":"session_closed","message":"session closed"}),
                ))
            }
        }
        let out = lab.out(&[
            "send",
            NAME,
            "not accepted",
            "--steer",
            "--request-id",
            "api-rejected",
        ]);
        fs::write(lab.path("canter-home/server.json"), registration).unwrap();
        let status = lab.status();
        server.close_all();
        lab.json(&["stop", NAME]);
        let sent: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(out.status.code(), Some(7), "{failure}: {sent}");
        assert_eq!(sent["error"], "steer_failed");
        assert_eq!(sent["written"], false);
        assert_eq!(sent["submitted"], false);
        assert_eq!(sent["confirmed"], false);
        assert_eq!(sent["request_id"], "api-rejected");
        assert_eq!(status["recent_sends"][0]["state"], "rejected");
        assert_eq!(status["recent_sends"][0]["written"], false);
        assert_eq!(
            server.state.lock().unwrap().inputs.len(),
            usize::from(failure == "rejected")
        );
    }
}

#[test]
fn steer_working_canter_never_retries_an_unknown_receipt_and_keeps_pen_responsive() {
    for failure in ["disconnect", "timeout", "invalid", "internal"] {
        let lab = Lab::new();
        let server = FakeServer::new(&lab, "unknown.sock");
        lab.working(&server);
        {
            let mut state = server.state.lock().unwrap();
            state.disconnect_input = failure == "disconnect";
            state.hold_input = failure == "timeout";
            if failure == "invalid" {
                state.input_reply = Some((200, json!({"unexpected":true})));
            }
            if failure == "internal" {
                state.input_reply =
                    Some((500, json!({"code":"internal","message":"internal error"})));
            }
        }
        let out = std::thread::scope(|scope| {
            let sending = scope.spawn(|| {
                lab.out(&[
                    "send",
                    NAME,
                    "only once",
                    "--steer",
                    "--request-id",
                    "api-unknown",
                ])
            });
            eventually(|| !server.state.lock().unwrap().inputs.is_empty());
            if failure == "timeout" {
                assert_eq!(lab.status()["recent_sends"][0]["state"], "accepted");
                assert_eq!(
                    lab.request(json!({"op":"upgrade","exe":lab.path("unused")}))["error"],
                    "upgrade_busy"
                );
                lab.json(&["keys", NAME, "text:still responsive"]);
                assert!(
                    !sending.is_finished(),
                    "pen waited for the API receipt before serving other requests"
                );
            }
            sending.join().unwrap()
        });
        let status = lab.status();
        server.state.lock().unwrap().hold_input = false;
        server.close_all();
        lab.json(&["stop", NAME]);
        let sent: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(out.status.code(), Some(3), "{failure}: {sent}");
        assert_eq!(sent["error"], "not_delivered");
        assert_eq!(sent.get("written"), Some(&Value::Null));
        assert_eq!(sent.get("submitted"), Some(&Value::Null));
        assert_eq!(sent["confirmed"], false);
        assert_eq!(sent["request_id"], "api-unknown");
        assert_eq!(status["recent_sends"][0]["state"], "unknown");
        assert_eq!(
            server.state.lock().unwrap().inputs.len(),
            1,
            "must never retry"
        );
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
fn a_quiet_session_switch_stays_idle_before_its_first_fifteen_second_heartbeat() {
    let lab = Lab::new();
    let server = FakeServer::new(&lab, "delayed-heartbeat.sock");
    server.state.lock().unwrap().delayed_heartbeat = true;
    lab.start(&["--", "canter"]);
    server.session("first", millis() as f64 / 1000. + 0.001);
    lab.publish("first");
    lab.wait_state("idle");
    eventually(|| {
        server
            .state
            .lock()
            .unwrap()
            .requests
            .iter()
            .any(|r| r.starts_with("/v1/events?session=first&"))
    });
    let first_contact = lab.request(json!({"op":"status"}))["watcher"]["last_contact"]
        .as_f64()
        .unwrap();
    // Switch before the first stream's heartbeat. The next stream is also silent for 15 seconds.
    std::thread::sleep(Duration::from_secs(10));
    server.session("quiet", 1.);
    lab.publish("quiet");
    eventually(|| {
        server
            .state
            .lock()
            .unwrap()
            .requests
            .iter()
            .any(|r| r.starts_with("/v1/events?session=quiet&"))
    });
    let until = Instant::now() + Duration::from_secs(12);
    while Instant::now() < until {
        assert_eq!(
            lab.status()["state"],
            "idle",
            "a connected quiet stream must not become unknown"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(
        lab.json(&["wait", NAME, "--timeout", "1"])["result"],
        "idle"
    );
    let connected_contact = lab.request(json!({"op":"status"}))["watcher"]["last_contact"]
        .as_f64()
        .unwrap();
    assert!(connected_contact > first_contact);
    // The delayed heartbeat subsequently advances the contact time again.
    eventually(|| {
        lab.request(json!({"op":"status"}))["watcher"]["last_contact"]
            .as_f64()
            .unwrap()
            > connected_contact
    });
    server.close_all();
    lab.json(&["stop", NAME]);
}

#[test]
fn malformed_sse_does_not_keep_old_status_alive_or_repeat_the_error_log() {
    let lab = Lab::new();
    let server = FakeServer::new(&lab, "malformed.sock");
    lab.start(&["--", "canter"]);
    server.session("s", millis() as f64 / 1000. + 0.001);
    lab.publish("s");
    lab.wait_state("idle");
    turn(&server, "s", "previous reply");
    eventually(|| {
        lab.records()
            .iter()
            .any(|e| e["last_assistant_message"] == "previous reply")
    });
    server.emit("s", "broken", json!({"malformed":true}));
    eventually(|| lab.request(json!({"op":"status"}))["watcher"]["error"].is_string());
    let contact = lab.request(json!({"op":"status"}))["watcher"]["last_contact"].clone();
    eventually(|| {
        server
            .state
            .lock()
            .unwrap()
            .requests
            .iter()
            .filter(|r| r.contains("after=4"))
            .count()
            >= 3
    });
    assert_eq!(
        lab.request(json!({"op":"status"}))["watcher"]["last_contact"],
        contact
    );
    let log = fs::read_to_string(lab.path("pens").join(NAME).join("pen.log")).unwrap();
    assert_eq!(
        log.lines()
            .filter(|line| line.starts_with("canter:"))
            .count(),
        1,
        "{log}"
    );
    server.close_all();
    lab.json(&["stop", NAME]);
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
                        if content.contains("r56b-retract") && !is_tool {
                            while !done.load(Ordering::SeqCst) { std::thread::sleep(Duration::from_millis(50)); }
                            return;
                        }
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
fn an_old_pen_that_paused_canter_can_still_be_resumed() {
    let lab = Lab::new();
    let dir = lab.path("pens").join(NAME);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("meta.json"),
        json!({"kind":"canter","instance":"old","proto":1}).to_string(),
    )
    .unwrap();
    let listener = UnixListener::bind(dir.join("sock")).unwrap();
    listener.set_nonblocking(true).unwrap();
    let done = Arc::new(AtomicBool::new(false));
    let stop = done.clone();
    let thread = std::thread::spawn(move || {
        let mut requests = Vec::new();
        while !stop.load(Ordering::SeqCst) {
            if let Ok((mut socket, _)) = listener.accept() {
                let mut line = String::new();
                BufReader::new(socket.try_clone().unwrap())
                    .read_line(&mut line)
                    .unwrap();
                let request: Value = serde_json::from_str(&line).unwrap();
                requests.push(request["op"].clone());
                writeln!(
                    socket,
                    "{}",
                    json!({"ok":true,"proto":1,"instance":"old","paused":request["op"] != "resume"})
                )
                .unwrap();
            } else {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        requests
    });
    let out = lab.out(&["resume", NAME]);
    done.store(true, Ordering::SeqCst);
    let requests = thread.join().unwrap();
    fs::remove_file(dir.join("sock")).unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["paused"],
        false
    );
    assert_eq!(
        requests,
        json!(["status", "resume"]).as_array().unwrap().clone()
    );
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
