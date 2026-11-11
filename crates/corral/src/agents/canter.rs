//! canter's public session file and HTTP/SSE integration (R56).
use super::{Adapter, Launch};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub(super) struct Canter;
impl Adapter for Canter {
    fn kind(&self) -> &'static str {
        "canter"
    }
    fn unsupported(&self) -> &'static [&'static str] {
        &["pause", "sandbox"]
    }
    fn watcher(&self) -> Option<&'static dyn Watcher> {
        Some(&Canter)
    }
    fn steer_api(&self) -> Option<super::SteerApi> {
        Some(steer_input)
    }
    fn answer_api(&self) -> Option<super::AnswerApi> {
        Some(answer_request)
    }
    fn validate_args(&self, args: &[String]) -> Result<()> {
        if args
            .iter()
            .take_while(|a| *a != "--")
            .any(|a| a == "--session-file" || a.starts_with("--session-file="))
        {
            return Err(Error::new(
                1,
                "usage",
                "corral supplies canter's --session-file",
            ));
        }
        Ok(())
    }
    fn hook_args(&self, launch: &Launch) -> Result<Vec<String>> {
        let mut args = vec![
            "--session-file".into(),
            launch
                .dir
                .join("canter-session.json")
                .to_string_lossy()
                .into_owned(),
        ];
        if !launch
            .args
            .iter()
            .take_while(|a| *a != "--")
            .any(|a| a == "--close-on-exit")
        {
            args.push("--close-on-exit".into());
        }
        Ok(args)
    }
    fn quit_steps(&self) -> Value {
        json!([{"signal":"HUP","wait":3},{"signal":"TERM","wait":3}])
    }
    fn skill_targets(&self) -> &'static [&'static str] {
        &["codex"]
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Translation {
    running: bool,
    turn: Option<String>,
    message: Option<String>,
    requests: Vec<Value>,
    agent_requests: Vec<(String, String)>,
    state: String,
}
impl Translation {
    fn interaction(&self) -> Value {
        let kind = self
            .requests
            .first()
            .and_then(|r| match r["kind"]["type"].as_str()? {
                "userInput" => Some("question"),
                "approval" | "sandboxApproval" | "planApproval" => Some("approval"),
                _ => None,
            });
        let mut interaction = json!({"kind":kind,"count":self.requests.len()+self.agent_requests.len(),"running":self.running});
        if self.agent_requests.is_empty()
            && let Some(request) = self.requests.first()
            && request["id"].as_str().is_some_and(|id| !id.is_empty())
        {
            let kind = &request["kind"];
            let mut detail = json!({"id":request["id"],"kind":kind["type"]});
            match kind["type"].as_str() {
                Some("approval" | "sandboxApproval") => {
                    detail["tool"] = kind["tool"].clone();
                    detail["available_decisions"] = kind["availableDecisions"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                        .into();
                    if matches!(kind["tool"].as_str(), Some("write" | "edit")) {
                        detail["file"] = crate::hooks::command(&kind["arguments"]["path"])
                            .unwrap_or(Value::Null);
                    }
                    match kind["tool"].as_str() {
                        Some("write") => {
                            detail["content"] =
                                crate::hooks::command(&kind["arguments"]["content"])
                                    .unwrap_or(Value::Null);
                        }
                        Some("edit") => {
                            detail["edits"] = kind["arguments"]["edits"]
                                .as_array()
                                .and_then(|edits| {
                                    crate::hooks::command(&json!(
                                        serde_json::to_string_pretty(edits).unwrap()
                                    ))
                                })
                                .unwrap_or(Value::Null);
                        }
                        _ => {}
                    }
                }
                Some("planApproval") => {
                    detail["plan"] = crate::hooks::command(&kind["plan"]).unwrap_or(Value::Null);
                }
                _ => {}
            }
            interaction["request"] = detail;
        }
        interaction
    }
    fn dialog(&self) -> Value {
        let Some(request) = self.requests.first() else {
            return if self.agent_requests.is_empty() {
                if self.running {
                    json!({"ev":"PostToolUse"})
                } else {
                    json!({"ev":"Stop","idle_only":true})
                }
            } else {
                json!({"ev":"PermissionRequest","tool_name":null})
            };
        };
        let kind = &request["kind"];
        let mut e = json!({"ev":"PermissionRequest","tool_name":kind["type"]});
        match kind["type"].as_str() {
            Some("approval") => {
                e["tool_name"] = kind["tool"].clone();
                if kind["tool"] == "shell" {
                    e["command"] =
                        crate::hooks::command(&kind["arguments"]["command"]).unwrap_or(Value::Null);
                }
            }
            Some("userInput") => {
                e["tool_name"] = json!("ask_user");
                e["questions"] = crate::hooks::questions(&json!([{"header":"","question":kind["question"],"multiSelect":false,"options":kind["options"]}])).unwrap_or(Value::Null);
            }
            _ => {}
        }
        e
    }
    fn event(&mut self, frame: &Value) -> Vec<Value> {
        let event = match frame["type"].as_str().unwrap_or("") {
            "turn.started" => {
                if self.turn.is_none() {
                    self.message = None;
                }
                self.running = true;
                self.turn = Some("running".into());
                let prompt: String = frame["turn"]["input"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|v| v["type"] == "text")
                    .filter_map(|v| v["text"].as_str())
                    .collect();
                Some(json!({"ev":"UserPromptSubmit","prompt":prompt}))
            }
            "item.completed" if frame["item"]["type"] == "agentMessage" => {
                if frame["item"]["status"] != "interrupted" {
                    self.message = frame["item"]["text"].as_str().map(str::to_owned);
                }
                None
            }
            "turn.completed" => {
                self.running = false;
                self.turn = frame["turn"]["status"].as_str().map(str::to_owned);
                None
            }
            "turn.retracted" => {
                self.running = false;
                self.turn = Some("interrupted".into());
                None
            }
            "request.opened" => {
                let r = &frame["request"];
                if !self.requests.iter().any(|old| old["id"] == r["id"]) {
                    self.requests.push(r.clone());
                    self.requests
                        .sort_by_key(|r| r["createdAt"].as_u64().unwrap_or(0));
                }
                Some(self.dialog())
            }
            "request.resolved" => {
                self.requests.retain(|r| r["id"] != frame["requestId"]);
                Some(self.dialog())
            }
            "agent.request.opened" => {
                let key = (
                    frame["agentId"].as_str().unwrap_or("").to_owned(),
                    frame["request"]["id"].as_str().unwrap_or("").to_owned(),
                );
                if !self.agent_requests.contains(&key) {
                    self.agent_requests.push(key);
                }
                Some(self.dialog())
            }
            "agent.request.resolved" => {
                self.agent_requests
                    .retain(|(a, r)| frame["agentId"] != *a || frame["requestId"] != *r);
                Some(self.dialog())
            }
            "item.started" => {
                let tool = match frame["item"]["type"].as_str() {
                    Some("commandExecution") => Some("shell"),
                    Some("fileRead") => Some("read"),
                    Some("fileChange") => Some("edit"),
                    Some("mcpToolCall") => frame["item"]["tool"].as_str(),
                    Some("webSearch") => Some("web_search"),
                    Some("webFetch") => Some("web_fetch"),
                    _ => None,
                };
                tool.map(|tool| json!({"ev":"PreToolUse","tool_name":tool}))
            }
            "session.status" if frame["status"] == "waiting_approval" => Some(
                if self.requests.is_empty() && self.agent_requests.is_empty() {
                    json!({"ev":"PermissionRequest","tool_name":null})
                } else {
                    self.dialog()
                },
            ),
            "session.status" if frame["status"] == "working" && self.state == "idle" => {
                Some(json!({"ev":"PreToolUse"}))
            }
            "session.status" if frame["status"] == "error" => {
                self.running = false;
                self.turn = None;
                Some(json!({"ev":"StopFailure"}))
            }
            "session.status"
                if frame["status"] == "closed" && frame["closeReason"] != "serverShutdown" =>
            {
                self.running = false;
                self.requests.clear();
                self.agent_requests.clear();
                Some(json!({"ev":"SessionEnd"}))
            }
            "session.status" if frame["status"] == "idle" => self
                .turn
                .take()
                .map(|result| {
                    let mut e = json!({"ev":match result.as_str() {
                        "completed" => "Stop", "interrupted" => "Interrupt", _ => "StopFailure"
                    }});
                    if result == "completed"
                        && let Some(text) = self.message.take()
                    {
                        e["last_assistant_message"] = json!(text);
                    }
                    e
                })
                .or_else(|| (self.state != "idle").then(|| json!({"ev":"Stop","idle_only":true}))),
            _ => None,
        };
        event
            .into_iter()
            .map(|mut e| {
                self.state = match e["ev"].as_str().unwrap_or("") {
                    "PermissionRequest" => "blocked",
                    "Stop" | "StopFailure" | "Interrupt" => "idle",
                    "SessionEnd" => "exiting",
                    _ => "working",
                }
                .into();
                e["interaction"] = self.interaction();
                e["t"] = json!(frame["timestamp"].as_f64().unwrap_or(0.0) / 1000.0);
                e
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context(dir: &Path) -> Context {
        use std::sync::{Arc, Mutex, atomic::AtomicBool};
        Context {
            dir: dir.into(),
            cwd: dir.display().to_string(),
            instance: "i".into(),
            started: 1.,
            shared: Arc::new(Mutex::new(watcher::State {
                cursor: json!({}),
                last_contact: 1.,
                error: None,
                finished: false,
                tracked: vec![],
                remaining: vec![],
            })),
            stop: Arc::new(AtomicBool::new(false)),
            exiting: Arc::new(AtomicBool::new(false)),
        }
    }
    #[test]
    fn replay_skips_ephemeral_duplicate_and_unknown_frames_and_feeds_the_existing_reader() {
        let dir = tempfile::tempdir().unwrap();
        let context = context(dir.path());
        let mut tracked = Tracked {
            session: Session {
                session_id: "s".into(),
                state_dir: dir.path().into(),
            },
            seq: 0,
            translation: Translation::default(),
            initialized: false,
        };
        initialize(
            &context,
            &mut tracked,
            &json!({"createdAt":2000,"lastEventSeq":0}),
        )
        .unwrap();
        let frames = [
            frame("turn.started", json!({"seq":0,"turn":{"input":[]}})),
            frame("turn.started", json!({"seq":1,"turn":{"input":[]}})),
            frame("turn.started", json!({"seq":1,"turn":{"input":[]}})),
            frame("future.event", json!({"seq":2})),
            frame("item.delta", json!({"seq":3})),
            frame(
                "item.completed",
                json!({"seq":4,"item":{"type":"agentMessage","text":"answer"}}),
            ),
            frame(
                "turn.completed",
                json!({"seq":5,"turn":{"status":"completed"}}),
            ),
            frame("session.status", json!({"seq":6,"status":"idle"})),
            frame("session.status", json!({"seq":6,"status":"idle"})),
        ];
        for frame in frames {
            consume(&context, &mut tracked, &frame).unwrap();
        }
        let state = crate::events::read(dir.path(), "i", dir.path(), false).unwrap();
        assert_eq!(tracked.seq, 6);
        assert_eq!(state["input_count"], 1);
        assert_eq!(state["state"], "idle");
        assert_eq!(state["reply"], "answer");
        assert_eq!(
            fs::read_to_string(dir.path().join("events"))
                .unwrap()
                .lines()
                .count(),
            3
        );
    }
    #[test]
    fn resumed_snapshot_keeps_current_dialogs_without_replaying_old_replies() {
        let dir = tempfile::tempdir().unwrap();
        let context = context(dir.path());
        let mut tracked = Tracked {
            session: Session {
                session_id: "s".into(),
                state_dir: dir.path().into(),
            },
            seq: 0,
            translation: Translation::default(),
            initialized: false,
        };
        initialize(&context,&mut tracked,&json!({"createdAt":0,"lastEventSeq":30,"status":"waiting_approval",
            "turns":[{"status":"inProgress","items":[]}],"pendingRequests":[{"id":"r","kind":{"type":"approval","tool":"shell","arguments":{"command":"\"雪\"\n".repeat(10000)}}}]})).unwrap();
        let state = crate::events::read(dir.path(), "i", dir.path(), false).unwrap();
        assert_eq!(tracked.seq, 30);
        assert_eq!(state["input_count"], 0);
        assert!(state["reply"].is_null());
        assert_eq!(state["state"], "blocked");
        assert_eq!(state["interaction"]["running"], true);
        assert_eq!(state["command"]["truncated"], true);
        assert!(state["command"].to_string().len() <= 16 * 1024);
        let mut translation = Translation::default();
        let q = translation.event(&frame("request.opened",json!({"request":{"id":"q","kind":{"type":"userInput","question":"雪".repeat(10000),"options":[{"label":"甲","description":"乙"}]}}})));
        assert_eq!(q[0]["questions"]["truncated"], true);
        assert!(q[0]["questions"].to_string().len() <= 16 * 1024);
    }
    fn frame(kind: &str, extra: Value) -> Value {
        let mut v = json!({"type":kind,"seq":1,"timestamp":1000,"sessionId":"s"});
        v.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        v
    }
    #[test]
    fn exiting_keeps_the_current_sessions_initialization_and_cursor() {
        let dir = tempfile::tempdir_in("/tmp").unwrap();
        let context = context(dir.path());
        let shared = context.shared.clone();
        let session = Session {
            session_id: "s".into(),
            state_dir: dir.path().into(),
        };
        fs::write(
            dir.path().join("canter-session.json"),
            serde_json::to_vec(&session).unwrap(),
        )
        .unwrap();
        let socket = dir.path().join("sock");
        fs::write(
            dir.path().join("server.json"),
            json!({"socketPath":socket}).to_string(),
        )
        .unwrap();
        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Read the GET before replying so macOS does not reset the closing connection.
            use std::io::BufRead;
            let mut reader = io::BufReader::new(&mut stream);
            let mut line = String::new();
            loop {
                line.clear();
                assert!(reader.read_line(&mut line).unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
            }
            let body = json!({"session":{"status":"closed"}}).to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        context.save(&Cursor {
            current: Some(session.clone()),
            sessions: vec![Tracked {
                session,
                seq: 42,
                translation: Translation::default(),
                initialized: true,
            }],
        });
        context.exiting.store(true, Ordering::SeqCst);
        Canter.run(context);
        server.join().unwrap();
        let state = watcher::lock(&shared);
        assert_eq!(state.cursor["sessions"][0]["initialized"], true);
        assert_eq!(state.cursor["sessions"][0]["seq"], 42);
        assert!(state.finished);
        assert!(state.remaining.is_empty());
    }
    #[test]
    fn sse_frames_and_streamed_json_have_independent_body_limits() {
        let mut bytes = vec![b'x'; 64 * 1024 * 1024];
        let mut scanned = 0;
        assert!(next_sse(&mut bytes, &mut scanned).unwrap().is_none());
        bytes.push(b'x');
        assert!(next_sse(&mut bytes, &mut scanned).is_err());
        bytes.extend_from_slice(b"\n\n");
        assert!(next_sse(&mut bytes, &mut scanned).is_err());
        drop(bytes);

        let dir = tempfile::tempdir_in("/tmp").unwrap();
        let context = context(dir.path());
        let socket = dir.path().join("sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let _ = (|| -> io::Result<()> {
                stream.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")?;
                // Individually legal chunks, but their combined JSON string exceeds 64 MiB.
                let chunk = vec![b'a'; 32 * 1024 * 1024];
                stream.write_all(b"1\r\n\"\r\n")?;
                for _ in 0..2 {
                    write!(stream, "{:x}\r\n", chunk.len())?;
                    stream.write_all(&chunk)?;
                    stream.write_all(b"\r\n")?;
                }
                stream.write_all(b"1\r\n\"\r\n0\r\n\r\n")
            })();
        });
        let error = get_json_until(
            &context,
            &socket,
            "/v1/info",
            Instant::now() + Duration::from_secs(10),
        )
        .unwrap_err();
        server.join().unwrap();
        assert!(
            error.to_string().contains("JSON body exceeds 64 MiB"),
            "{error}"
        );
    }
    #[test]
    fn oversized_http_chunks_are_rejected_before_reading_their_body() {
        let (socket, _peer) = UnixStream::pair().unwrap();
        let mut response = Http {
            socket,
            status: 200,
            buffer: b"4000001\r\n".to_vec(),
            chunked: true,
            left: None,
            done: false,
        };
        assert!(response.decoded().is_err());
        response.buffer = vec![b'f'; 65537];
        assert!(
            response.decoded().is_err(),
            "unterminated chunk header must also be bounded"
        );
    }
    #[test]
    fn discovery_files_reject_links_fifos_and_oversized_contents() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["canter-session.json", "server.json"] {
            let path = dir.path().join(name);
            let target = dir.path().join("target");
            let json =
                json!({"sessionId":"s","stateDir":dir.path(),"socketPath":dir.path().join("sock")})
                    .to_string();
            fs::write(&target, &json).unwrap();
            let read = || {
                if name == "server.json" {
                    discover(dir.path()).map(|_| ())
                } else {
                    published(dir.path()).map(|_| ())
                }
            };
            std::os::unix::fs::symlink(&target, &path).unwrap();
            assert!(read().is_err(), "must not follow {name}");
            fs::remove_file(&path).unwrap();
            use std::os::unix::ffi::OsStrExt;
            let cpath = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
            assert_eq!(unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) }, 0);
            assert!(read().is_err(), "must not block on {name}");
            fs::remove_file(&path).unwrap();
            fs::write(&path, format!("{json}{}", " ".repeat(65536))).unwrap();
            assert!(read().is_err(), "must bound {name}");
            fs::write(&path, &json).unwrap();
            read().unwrap();
        }
    }
    #[test]
    fn idle_child_dialogs_and_work_without_a_turn_return_to_idle_without_a_new_reply() {
        let dir = tempfile::tempdir().unwrap();
        let context = context(dir.path());
        let mut tracked = Tracked {
            session: Session {
                session_id: "s".into(),
                state_dir: dir.path().into(),
            },
            seq: 0,
            translation: Translation::default(),
            initialized: false,
        };
        initialize(&context, &mut tracked, &json!({"createdAt":2000})).unwrap();
        let frames = [
            frame("turn.started", json!({"turn":{"input":[]}})),
            frame(
                "item.completed",
                json!({"item":{"type":"agentMessage","text":"previous reply"}}),
            ),
            frame("turn.completed", json!({"turn":{"status":"completed"}})),
            frame("session.status", json!({"status":"idle"})),
            frame("turn.started", json!({"turn":{"input":[]}})),
            frame("turn.retracted", json!({})),
            frame("session.status", json!({"status":"idle"})),
            frame(
                "agent.request.opened",
                json!({"agentId":"a","request":{"id":"r"}}),
            ),
            frame(
                "agent.request.resolved",
                json!({"agentId":"a","requestId":"r"}),
            ),
            frame("session.status", json!({"status":"working"})),
            frame("session.status", json!({"status":"idle"})),
        ];
        for (index, mut f) in frames.into_iter().enumerate() {
            f["seq"] = json!(index + 1);
            consume(&context, &mut tracked, &f).unwrap();
            if index == 8 || index == 10 {
                let state = crate::events::read(dir.path(), "i", dir.path(), false).unwrap();
                assert_eq!(state["state"], "idle");
                assert_eq!(state["interrupted"], true);
                assert_eq!(state["reply"], "previous reply");
                assert_eq!(state["interaction"]["count"], 0);
            }
        }
        for status in ["working", "waiting_approval"] {
            initialize(&context, &mut tracked, &json!({"createdAt":0,"lastEventSeq":40,"status":status,
                "pendingRequests":if status == "working" { json!([]) } else { json!([{"id":"r","kind":{"type":"approval"}}]) }
            })).unwrap();
            assert_eq!(
                tracked.translation.state,
                if status == "working" {
                    "working"
                } else {
                    "blocked"
                }
            );
            let end = tracked
                .translation
                .event(&frame("session.status", json!({"status":"idle"})));
            assert_eq!(end[0]["ev"], "Stop");
            assert_eq!(end[0]["idle_only"], true);
        }
    }
    #[test]
    fn retract_before_the_first_model_word_is_an_interruption() {
        let mut t = Translation::default();
        t.event(&frame("turn.started", json!({"turn":{"input":[]}})));
        t.event(&frame("session.status", json!({"status":"working"})));
        t.event(&frame(
            "item.completed",
            json!({"item":{"type":"userMessage"}}),
        ));
        assert!(
            t.event(&frame("turn.retracted", json!({"turnId":"t"})))
                .is_empty()
        );
        let end = t.event(&frame("session.status", json!({"status":"idle"})));
        assert_eq!(end[0]["ev"], "Interrupt");
        assert_eq!(end[0]["interaction"]["running"], false);
        assert!(end[0].get("last_assistant_message").is_none());
    }
    #[test]
    fn translates_a_turn_only_when_the_session_becomes_idle() {
        let mut t = Translation::default();
        let start = t.event(&frame(
            "turn.started",
            json!({"turn":{"id":"t","input":[{"type":"text","text":"one\ntwo"}]}}),
        ));
        assert_eq!(start[0]["ev"], "UserPromptSubmit");
        assert_eq!(start[0]["prompt"], "one\ntwo");
        assert!(
            t.event(&frame(
                "item.completed",
                json!({"item":{"type":"agentMessage","id":"a","text":"answer"}})
            ))
            .is_empty()
        );
        assert!(
            t.event(&frame(
                "turn.completed",
                json!({"turn":{"id":"t","status":"completed"}})
            ))
            .is_empty()
        );
        let end = t.event(&frame("session.status", json!({"status":"idle"})));
        assert_eq!(end[0]["ev"], "Stop");
        assert_eq!(end[0]["last_assistant_message"], "answer");
        assert_eq!(
            end[0]["interaction"],
            json!({"kind":null,"count":0,"running":false})
        );
        assert!(
            t.event(&frame("session.status", json!({"status":"idle"})))
                .is_empty()
        );
    }
    #[test]
    fn requests_preserve_original_commands_questions_and_count_child_dialogs() {
        let mut t = Translation::default();
        t.event(&frame("turn.started", json!({"turn":{"input":[]}})));
        let opened = t.event(&frame("request.opened", json!({"request":{"id":"r","createdAt":1000,"kind":{"type":"approval","tool":"shell","arguments":{"command":"echo 'one'\n echo two"}}}})));
        assert_eq!(opened[0]["ev"], "PermissionRequest");
        assert_eq!(opened[0]["tool_name"], "shell");
        assert_eq!(
            opened[0]["command"],
            json!({"text":"echo 'one'\n echo two","truncated":false})
        );
        assert_eq!(
            opened[0]["interaction"],
            json!({"kind":"approval","count":1,"running":true,"request":{"id":"r","kind":"approval","tool":"shell","available_decisions":[]}})
        );
        let question = json!({"id":"q","createdAt":2000,"kind":{"type":"userInput","question":"选哪个？","options":[{"label":"甲","description":"说明"}]}});
        let second = t.event(&frame("request.opened", json!({"request":question})));
        assert_eq!(second[0]["command"], opened[0]["command"]);
        let next = t.event(&frame("request.resolved", json!({"requestId":"r"})));
        assert_eq!(
            next[0]["questions"],
            json!({"items":[{"header":"","question":"选哪个？","multi_select":false,"options":[{"label":"甲","description":"说明"}]}],"truncated":false})
        );
        t.event(&frame(
            "agent.request.opened",
            json!({"agentId":"a","request":{"id":"c"}}),
        ));
        let child = t.event(&frame("request.resolved", json!({"requestId":"q"})));
        assert_eq!(
            child[0]["interaction"],
            json!({"kind":null,"count":1,"running":true})
        );
        assert!(child[0].get("questions").is_none());
        let resolved = t.event(&frame(
            "agent.request.resolved",
            json!({"agentId":"a","requestId":"c"}),
        ));
        assert_eq!(resolved[0]["ev"], "PostToolUse");
    }
    #[test]
    fn interruption_failure_continuation_and_server_shutdown_do_not_publish_partial_replies() {
        for (status, expected) in [("interrupted", "Interrupt"), ("failed", "StopFailure")] {
            let mut t = Translation::default();
            t.event(&frame("turn.started", json!({"turn":{"input":[]}})));
            t.event(&frame(
                "item.completed",
                json!({"item":{"type":"agentMessage","text":"partial"}}),
            ));
            t.event(&frame("turn.completed", json!({"turn":{"status":status}})));
            let end = t.event(&frame("session.status", json!({"status":"idle"})));
            assert_eq!(end[0]["ev"], expected);
            assert!(end[0].get("last_assistant_message").is_none());
        }
        let mut t = Translation::default();
        t.event(&frame("turn.started", json!({"turn":{"input":[]}})));
        t.event(&frame(
            "turn.completed",
            json!({"turn":{"status":"completed"}}),
        ));
        assert!(
            t.event(&frame(
                "session.status",
                json!({"status":"closed","closeReason":"serverShutdown"})
            ))
            .is_empty()
        );
        let next = t.event(&frame("turn.started", json!({"turn":{"input":[]}})));
        assert_eq!(next[0]["ev"], "UserPromptSubmit");
        let close = t.event(&frame(
            "session.status",
            json!({"status":"closed","closeReason":"clientRequest"}),
        ));
        assert_eq!(close[0]["ev"], "SessionEnd");
    }
}

use crate::pen::watcher::{self, Context, Watcher};
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    os::unix::{fs::OpenOptionsExt, net::UnixStream},
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

const BODY_LIMIT: usize = 64 * 1024 * 1024;
const HEADER_LIMIT: usize = 64 * 1024;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Session {
    session_id: String,
    state_dir: PathBuf,
}
#[derive(Serialize, Deserialize)]
struct Tracked {
    session: Session,
    seq: u64,
    translation: Translation,
    initialized: bool,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Cursor {
    current: Option<Session>,
    sessions: Vec<Tracked>,
}
impl Context {
    fn cancelled(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }
    fn contact(&self) {
        let mut state = watcher::lock(&self.shared);
        state.last_contact = crate::now();
        state.error = None;
    }
    fn failed(&self, error: impl ToString) {
        let text = error.to_string();
        let mut state = watcher::lock(&self.shared);
        if state.error.as_ref() != Some(&text) {
            eprintln!("canter: {text}");
        }
        state.error = Some(text);
    }
    fn save(&self, cursor: &Cursor) {
        let mut state = watcher::lock(&self.shared);
        state.cursor = serde_json::to_value(cursor).unwrap();
        state.tracked = cursor
            .sessions
            .iter()
            .map(|s| s.session.session_id.clone())
            .collect();
        state.remaining = state.tracked.clone();
    }
}
impl Watcher for Canter {
    fn run(&self, context: Context) {
        let mut cursor: Cursor =
            serde_json::from_value(watcher::lock(&context.shared).cursor.clone())
                .unwrap_or_default();
        context.save(&cursor);
        let mut stream: Option<Http> = None;
        let mut sse = Vec::new();
        let mut scanned = 0;
        // After invalid data, only a new valid event can establish progress again.
        let mut allow_heartbeat = true;
        let mut retry = Instant::now();
        let mut next_session_check = Instant::now();
        while !context.cancelled() {
            if context.exiting.load(Ordering::SeqCst) {
                // Capture a publication that raced the terminal's exit, even before first connect.
                if let Ok(session) = published(&context.dir)
                    && cursor.current.as_ref() != Some(&session)
                {
                    track(&mut cursor, session);
                }
                context.save(&cursor);
                finish(&context, &cursor);
                return;
            }
            if Instant::now() >= next_session_check {
                next_session_check = Instant::now() + Duration::from_millis(500);
                match published(&context.dir) {
                    Ok(session) => {
                        if cursor.current.as_ref() != Some(&session) {
                            track(&mut cursor, session);
                            stream = None;
                            sse.clear();
                            scanned = 0;
                            allow_heartbeat = true;
                            retry = Instant::now();
                            context.save(&cursor);
                        }
                    }
                    Err(e) => {
                        if cursor.current.is_none() && e.kind() == io::ErrorKind::NotFound {
                            // Trust dialog: no session exists yet, however long the user takes.
                            context.contact();
                        } else {
                            context.failed(format!("session file: {e}"));
                        }
                    }
                }
            }
            if cursor.current.is_none() || Instant::now() < retry {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            let current = cursor.current.clone().unwrap();
            let tracked = cursor
                .sessions
                .iter_mut()
                .find(|s| s.session == current)
                .unwrap();
            let before = (tracked.seq, tracked.initialized);
            let result = (|| -> io::Result<()> {
                if stream.is_none() {
                    let socket = discover(&current.state_dir)?;
                    let info = get_json(&context, &socket, "/v1/info")?;
                    if info["protocolVersion"] != 1
                        || !info["features"]
                            .as_array()
                            .is_some_and(|a| a.iter().any(|f| f == "managedClients"))
                    {
                        return Err(io::Error::other(
                            "canter too old or unsupported protocol (managedClients required)",
                        ));
                    }
                    if !tracked.initialized {
                        let snapshot = get_json(
                            &context,
                            &socket,
                            &format!("/v1/sessions/{}", encode(&current.session_id)),
                        )?;
                        initialize(&context, tracked, &snapshot["session"])?;
                    }
                    stream = Some(Http::get(
                        &context,
                        &socket,
                        &format!(
                            "/v1/events?session={}&observe=true&after={}",
                            encode(&current.session_id),
                            tracked.seq
                        ),
                        Instant::now() + Duration::from_secs(2),
                    )?);
                    if allow_heartbeat {
                        context.contact();
                    }
                }
                let Some(bytes) = stream.as_mut().unwrap().next().inspect_err(|e| {
                    if e.kind() == io::ErrorKind::InvalidData {
                        allow_heartbeat = false;
                    }
                })?
                else {
                    return Err(io::Error::other("event stream ended; reconnecting"));
                };
                sse.extend(bytes.into_iter().filter(|b| *b != b'\r'));
                while let Some(frame) = next_sse(&mut sse, &mut scanned).inspect_err(|_| {
                    allow_heartbeat = false;
                })? {
                    let data = frame
                        .split(|b| *b == b'\n')
                        .filter_map(|l| l.strip_prefix(b"data:"))
                        .flat_map(|l| l.strip_prefix(b" ").unwrap_or(l).iter().copied())
                        .collect::<Vec<_>>();
                    if data.is_empty() {
                        if allow_heartbeat {
                            context.contact();
                        }
                        continue;
                    }
                    let frame: Value = serde_json::from_slice(&data).map_err(|e| {
                        allow_heartbeat = false;
                        io::Error::other(format!("invalid SSE event: {e}"))
                    })?;
                    if frame["seq"].as_u64().is_some_and(|seq| seq > tracked.seq)
                        && frame["sessionId"] == tracked.session.session_id
                    {
                        allow_heartbeat = true;
                    }
                    consume(&context, tracked, &frame)?;
                    if allow_heartbeat {
                        context.contact();
                    }
                    if frame["type"] == "session.status"
                        && frame["status"] == "closed"
                        && frame["closeReason"] == "serverShutdown"
                    {
                        return Err(io::Error::other("server shutdown; reconnecting"));
                    }
                }
                Ok(())
            })();
            if before != (tracked.seq, tracked.initialized) {
                context.save(&cursor);
            }
            if let Err(e) = result {
                context.failed(e);
                stream = None;
                sse.clear();
                scanned = 0;
                retry = Instant::now() + Duration::from_secs(1);
            }
        }
        context.save(&cursor);
    }
}
fn published(dir: &Path) -> io::Result<Session> {
    let session: Session =
        serde_json::from_slice(&read_public_file(&dir.join("canter-session.json"))?)
            .map_err(io::Error::other)?;
    if session.session_id.is_empty() || !session.state_dir.is_absolute() {
        return Err(io::Error::other("invalid session file"));
    }
    Ok(session)
}

fn steer_failed(error: impl std::fmt::Display) -> Error {
    Error::new(
        7,
        "steer_failed",
        format!("input not accepted: {error}; wait for idle before retrying"),
    )
    .with("written", false)
    .with("submitted", false)
    .with("confirmed", false)
}

fn steer_unknown(error: impl std::fmt::Display) -> Error {
    Error::new(
        3,
        "not_delivered",
        format!("input receipt unknown: {error}; do not resend"),
    )
    .with("written", Value::Null)
    .with("submitted", Value::Null)
    .with("confirmed", false)
}

fn steer_input(dir: &Path, expected_session: &str, text: &str) -> Result<Value> {
    let session = published(dir).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            Error::new(
                9,
                "unsupported_steer",
                "no published session for working input; wait for idle and use ordinary send",
            )
            .with("written", false)
            .with("submitted", false)
            .with("confirmed", false)
        } else {
            steer_failed(e)
        }
    })?;
    if session.session_id != expected_session {
        return Err(
            Error::new(7, "state_changed", "main session changed; no input written")
                .with("written", false)
                .with("submitted", false)
                .with("confirmed", false),
        );
    }
    let (status, receipt) = post_json(
        &session,
        "input",
        json!({"input":[{"type":"text","text":text}],"mode":"steer","clientName":"corral"}),
        steer_failed,
        steer_unknown,
    )?;
    if (400..500).contains(&status)
        && let (Some(code), Some(message)) = (receipt["code"].as_str(), receipt["message"].as_str())
    {
        return Err(steer_failed(format!("{code}: {message}")));
    }
    if status != 200
        || !receipt["eventSeq"].is_u64()
        || !(receipt["turnId"].is_null() || receipt["turnId"].is_string())
    {
        return Err(steer_unknown("invalid input response"));
    }
    Ok(
        json!({"ok":true,"written":true,"submitted":true,"confirmed":true,"merged_with_draft":false,
        "queued_for_next_turn":receipt["turnId"].as_str().is_none_or(str::is_empty)}),
    )
}

fn answer_failed(error: impl std::fmt::Display) -> Error {
    Error::new(7, "answer_failed", format!("answer not accepted: {error}"))
        .with("written", false)
        .with("submitted", false)
        .with("confirmed", false)
}

fn answer_unknown(error: impl std::fmt::Display) -> Error {
    Error::new(
        3,
        "not_delivered",
        format!("answer receipt unknown: {error}; do not retry"),
    )
    .with("written", Value::Null)
    .with("submitted", Value::Null)
    .with("confirmed", false)
}

fn answer_request(
    dir: &Path,
    expected_session: &str,
    request: &Value,
    answer: &Value,
) -> Result<Value> {
    let body = match answer["type"].as_str() {
        Some("option")
            if request["kind"] == "userInput"
                && answer["text"].as_str().is_some_and(|s| !s.is_empty()) =>
        {
            json!({"answer":{"type":"option","label":answer["text"]},"clientName":"corral"})
        }
        Some("text")
            if request["kind"] == "userInput"
                && answer["text"]
                    .as_str()
                    .is_some_and(|s| !s.trim().is_empty() && !s.contains(['\r', '\n'])) =>
        {
            json!({"answer":{"type":"text","text":answer["text"]},"clientName":"corral"})
        }
        Some("allow_once")
            if request["kind"] == "approval"
                && matches!(request["tool"].as_str(), Some("shell" | "write" | "edit"))
                && request["available_decisions"]
                    .as_array()
                    .is_some_and(|ds| ds.iter().any(|d| d == "accept")) =>
        {
            json!({"decision":"accept","clientName":"corral"})
        }
        Some("approve_plan") if request["kind"] == "planApproval" => {
            json!({"plan":{"decision":"implement","approvalPolicy":"alwaysAsk"},"clientName":"corral"})
        }
        _ => return Err(answer_failed("unsupported answer for this request")),
    };
    let session = published(dir).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            Error::new(9, "unsupported", "no published session for answering")
                .with("written", false)
                .with("submitted", false)
                .with("confirmed", false)
        } else {
            answer_failed(e)
        }
    })?;
    if session.session_id != expected_session {
        return Err(Error::new(
            7,
            "request_changed",
            "main session changed; answer withheld",
        )
        .with("written", false)
        .with("submitted", false)
        .with("confirmed", false));
    }
    let (status, receipt) = post_json(
        &session,
        &format!("requests/{}", encode(request["id"].as_str().unwrap_or(""))),
        body,
        answer_failed,
        answer_unknown,
    )?;
    if (400..500).contains(&status)
        && let (Some(code), Some(message)) = (receipt["code"].as_str(), receipt["message"].as_str())
    {
        return Err(answer_failed(format!("{code}: {message}")));
    }
    if status != 200 || !receipt["eventSeq"].is_u64() || !receipt["alreadyResolved"].is_boolean() {
        return Err(answer_unknown("invalid answer response"));
    }
    Ok(
        json!({"ok":true,"written":true,"submitted":true,"confirmed":true,"already_resolved":receipt["alreadyResolved"]}),
    )
}

fn post_json(
    session: &Session,
    suffix: &str,
    body: Value,
    failed: fn(String) -> Error,
    unknown: fn(String) -> Error,
) -> Result<(u16, Value)> {
    let socket = discover(&session.state_dir).map_err(|e| failed(e.to_string()))?;
    let mut socket = connect(&socket).map_err(|e| failed(e.to_string()))?;
    socket
        .set_read_timeout(Some(Duration::from_millis(500)))
        .map_err(|e| failed(e.to_string()))?;
    socket
        .set_write_timeout(Some(Duration::from_millis(500)))
        .map_err(|e| failed(e.to_string()))?;
    let body = body.to_string();
    let path = format!("/v1/sessions/{}/{suffix}", encode(&session.session_id));
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut written = 0;
    while written < request.len() {
        let result = if Instant::now() >= deadline {
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "input write deadline",
            ))
        } else {
            socket.write(&request.as_bytes()[written..])
        };
        match result {
            Ok(0) => {
                return Err(if written == 0 {
                    failed("socket closed".into())
                } else {
                    unknown("socket closed".into())
                });
            }
            Ok(n) => written += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                return Err(if written == 0 {
                    failed(e.to_string())
                } else {
                    unknown(e.to_string())
                });
            }
        }
    }
    let response =
        Http::response(socket, deadline, || false).map_err(|e| unknown(e.to_string()))?;
    let status = response.status;
    let receipt = response
        .json(deadline, || false)
        .map_err(|e| unknown(e.to_string()))?;
    Ok((status, receipt))
}

fn track(cursor: &mut Cursor, session: Session) {
    if !cursor.sessions.iter().any(|s| s.session == session) {
        cursor.sessions.push(Tracked {
            session: session.clone(),
            seq: 0,
            translation: Translation::default(),
            initialized: false,
        });
    } else {
        // Returning to a previously selected session starts from its current snapshot, not its old replies.
        let tracked = cursor
            .sessions
            .iter_mut()
            .find(|s| s.session == session)
            .unwrap();
        tracked.initialized = false;
    }
    cursor.current = Some(session);
}
fn discover(dir: &Path) -> io::Result<PathBuf> {
    let registration: Value = serde_json::from_slice(&read_public_file(&dir.join("server.json"))?)
        .map_err(io::Error::other)?;
    registration["socketPath"]
        .as_str()
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| io::Error::other("server.json has no absolute socketPath"))
}
fn read_public_file(path: &Path) -> io::Result<Vec<u8>> {
    const LIMIT: usize = 64 * 1024;
    if !fs::symlink_metadata(path)?.is_file() {
        return Err(io::Error::other("expected a regular file"));
    }
    // Recheck the opened file, without following a swapped symlink or blocking on a FIFO.
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("expected a regular file"));
    }
    let mut bytes = Vec::new();
    file.take((LIMIT + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > LIMIT {
        return Err(io::Error::other("discovery file exceeds 64 KiB"));
    }
    Ok(bytes)
}
fn record(context: &Context, tracked: &Tracked, mut event: Value, seq: u64) -> io::Result<()> {
    event["v"] = json!(1);
    event["inst"] = json!(context.instance);
    event["cwd"] = json!(context.cwd);
    event["session_id"] = json!(tracked.session.session_id);
    event["has_transcript"] = json!(true);
    event["canter_seq"] = json!(seq);
    let mut data = serde_json::to_vec(&event).map_err(io::Error::other)?;
    data.push(b'\n');
    OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o600)
        .open(context.dir.join("events"))?
        .write_all(&data)
}
fn initialize(context: &Context, tracked: &mut Tracked, snapshot: &Value) -> io::Result<()> {
    let created = snapshot["createdAt"]
        .as_f64()
        .ok_or_else(|| io::Error::other("snapshot missing createdAt"))?
        / 1000.;
    let resume = created < context.started || tracked.seq > 0;
    tracked.translation = Translation {
        state: "idle".into(),
        ..Translation::default()
    };
    if resume {
        tracked.seq = snapshot["lastEventSeq"]
            .as_u64()
            .ok_or_else(|| io::Error::other("snapshot missing lastEventSeq"))?;
        tracked.translation.requests = snapshot["pendingRequests"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        tracked
            .translation
            .requests
            .sort_by_key(|r| r["createdAt"].as_u64().unwrap_or(0));
        for request in snapshot["agentRequests"].as_array().into_iter().flatten() {
            tracked.translation.agent_requests.push((
                request["agentId"].as_str().unwrap_or("").into(),
                request["request"]["id"].as_str().unwrap_or("").into(),
            ));
        }
        if let Some(turn) = snapshot["turns"]
            .as_array()
            .and_then(|ts| ts.last())
            .filter(|t| t["status"] == "inProgress")
        {
            tracked.translation.running = true;
            tracked.translation.turn = Some("running".into());
            tracked.translation.message = turn["items"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|i| i["type"] == "agentMessage" && i["status"] != "interrupted")
                .filter_map(|i| i["text"].as_str())
                .next_back()
                .map(str::to_owned);
        }
    }
    record(
        context,
        tracked,
        json!({"ev":"SessionStart","t":crate::now(),"interaction":tracked.translation.interaction()}),
        tracked.seq,
    )?;
    if resume {
        let mut event = if !tracked.translation.requests.is_empty()
            || !tracked.translation.agent_requests.is_empty()
        {
            Some(tracked.translation.dialog())
        } else if snapshot["status"] == "working" {
            Some(json!({"ev":"PreToolUse"}))
        } else {
            None
        };
        if let Some(ref mut event) = event {
            tracked.translation.state = if event["ev"] == "PermissionRequest" {
                "blocked"
            } else {
                "working"
            }
            .into();
            event["t"] = json!(crate::now());
            event["interaction"] = tracked.translation.interaction();
            record(context, tracked, event.clone(), tracked.seq)?;
        }
    }
    tracked.initialized = true;
    Ok(())
}
fn consume(context: &Context, tracked: &mut Tracked, frame: &Value) -> io::Result<()> {
    let seq = frame["seq"].as_u64().unwrap_or(0);
    if seq == 0 || seq <= tracked.seq || frame["sessionId"] != tracked.session.session_id {
        return Ok(());
    }
    for event in tracked.translation.event(frame) {
        record(context, tracked, event, seq)?;
    }
    tracked.seq = seq;
    Ok(())
}
fn finish(context: &Context, cursor: &Cursor) {
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut remaining = cursor.sessions.iter().collect::<Vec<_>>();
    while !remaining.is_empty() && !context.cancelled() && Instant::now() < deadline {
        remaining.retain(|tracked| {
            if context.cancelled() || Instant::now() >= deadline {
                return true;
            }
            let status = discover(&tracked.session.state_dir).and_then(|socket| {
                get_json_until(
                    context,
                    &socket,
                    &format!("/v1/sessions/{}", encode(&tracked.session.session_id)),
                    deadline.min(Instant::now() + Duration::from_secs(2)),
                )
            });
            match status {
                Ok(v) if v["session"]["status"] == "closed" => false,
                Ok(v) => {
                    context.failed(format!(
                        "session {} still {}",
                        tracked.session.session_id, v["session"]["status"]
                    ));
                    true
                }
                Err(e) => {
                    context.failed(format!("server unreachable: {e}"));
                    true
                }
            }
        });
        if !remaining.is_empty() {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    let mut shared = watcher::lock(&context.shared);
    shared.remaining = remaining
        .iter()
        .map(|s| s.session.session_id.clone())
        .collect();
    shared.finished = true;
}
fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

fn next_sse(buffer: &mut Vec<u8>, scanned: &mut usize) -> io::Result<Option<Vec<u8>>> {
    if let Some(end) = buffer[*scanned..].windows(2).position(|w| w == b"\n\n") {
        let length = *scanned + end + 2;
        if length > BODY_LIMIT {
            return Err(io::Error::other("SSE frame exceeds 64 MiB"));
        }
        *scanned = 0;
        Ok(Some(buffer.drain(..length).collect()))
    } else {
        if buffer.len() > BODY_LIMIT {
            return Err(io::Error::other("SSE frame exceeds 64 MiB"));
        }
        *scanned = buffer.len().saturating_sub(1);
        Ok(None)
    }
}

/// Minimal HTTP/1.1 over the public Unix socket. Chunk boundaries are unrelated to SSE frames.
struct Http {
    socket: UnixStream,
    status: u16,
    buffer: Vec<u8>,
    chunked: bool,
    left: Option<usize>,
    done: bool,
}
impl Http {
    fn get(context: &Context, socket: &Path, path: &str, deadline: Instant) -> io::Result<Self> {
        let mut socket = connect(socket).map_err(|e| io::Error::other(format!("connect: {e}")))?;
        socket
            .set_read_timeout(Some(Duration::from_millis(500)))
            .map_err(|e| io::Error::other(format!("read timeout: {e}")))?;
        socket
            .set_write_timeout(Some(Duration::from_millis(500)))
            .map_err(|e| io::Error::other(format!("write timeout: {e}")))?;
        write!(
            socket,
            "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
        )
        .map_err(|e| io::Error::other(format!("GET write: {e}")))?;
        let response = Self::response(socket, deadline, || context.cancelled())?;
        if response.status != 200 {
            return Err(io::Error::other(format!("HTTP {}", response.status)));
        }
        Ok(response)
    }
    fn response(
        mut socket: UnixStream,
        deadline: Instant,
        cancelled: impl Fn() -> bool,
    ) -> io::Result<Self> {
        let mut buffer = Vec::new();
        let end = loop {
            if cancelled() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "observer stopped",
                ));
            }
            if let Some(end) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                break end;
            }
            if Instant::now() >= deadline || buffer.len() > 65536 {
                return Err(io::Error::other("HTTP header deadline or limit exceeded"));
            }
            let mut bytes = [0; 8192];
            match socket.read(&mut bytes) {
                Ok(0) => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "HTTP headers")),
                Ok(n) => buffer.extend_from_slice(&bytes[..n]),
                Err(e) if timed_out(&e) => {}
                Err(e) => return Err(e),
            }
        };
        let headers = String::from_utf8_lossy(&buffer[..end]);
        let status = headers
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| io::Error::other("invalid HTTP status"))?
            .parse()
            .map_err(io::Error::other)?;
        let mut chunked = false;
        let mut left = None;
        for line in headers.lines().skip(1) {
            if let Some((key, value)) = line.split_once(':') {
                if key.eq_ignore_ascii_case("content-length") {
                    left = Some(value.trim().parse::<usize>().map_err(io::Error::other)?);
                }
                if key.eq_ignore_ascii_case("transfer-encoding") {
                    chunked = value.trim().eq_ignore_ascii_case("chunked");
                }
            }
        }
        buffer.drain(..end + 4);
        Ok(Self {
            socket,
            status,
            buffer,
            chunked,
            left,
            done: false,
        })
    }
    fn json(mut self, deadline: Instant, cancelled: impl Fn() -> bool) -> io::Result<Value> {
        if self.left.is_some_and(|length| length > BODY_LIMIT) {
            return Err(io::Error::other("JSON body exceeds 64 MiB"));
        }
        let mut bytes = Vec::new();
        while !cancelled() && Instant::now() < deadline {
            match self.next()? {
                Some(chunk) => {
                    if chunk.len() > BODY_LIMIT - bytes.len() {
                        return Err(io::Error::other("JSON body exceeds 64 MiB"));
                    }
                    bytes.extend(chunk);
                }
                None => return serde_json::from_slice(&bytes).map_err(io::Error::other),
            }
        }
        Err(io::Error::other("JSON request cancelled or timed out"))
    }
    fn decoded(&mut self) -> io::Result<Option<Vec<u8>>> {
        if self.chunked {
            let Some(end) = self.buffer.windows(2).position(|w| w == b"\r\n") else {
                if self.buffer.len() > HEADER_LIMIT {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "chunk header exceeds 64 KiB",
                    ));
                }
                return Ok(None);
            };
            if end > HEADER_LIMIT {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "chunk header exceeds 64 KiB",
                ));
            }
            let line = std::str::from_utf8(&self.buffer[..end])
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            let size = usize::from_str_radix(line.split(';').next().unwrap().trim(), 16)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            if size > BODY_LIMIT {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "HTTP chunk exceeds 64 MiB",
                ));
            }
            if size == 0 {
                self.done = true;
                return Ok(Some(vec![]));
            }
            let tail = (end + 2)
                .checked_add(size)
                .and_then(|n| n.checked_add(2))
                .ok_or_else(|| io::Error::other("chunk size overflow"))?;
            if self.buffer.len() < tail {
                return Ok(None);
            }
            if &self.buffer[tail - 2..tail] != b"\r\n" {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid chunk terminator",
                ));
            }
            let bytes = self.buffer[end + 2..tail - 2].to_vec();
            self.buffer.drain(..tail);
            Ok(Some(bytes))
        } else {
            let n = self
                .left
                .unwrap_or(self.buffer.len())
                .min(self.buffer.len());
            if let Some(left) = &mut self.left {
                *left -= n;
                self.done = *left == 0;
            }
            if n == 0 {
                return Ok(None);
            }
            Ok(Some(self.buffer.drain(..n).collect()))
        }
    }
    fn next(&mut self) -> io::Result<Option<Vec<u8>>> {
        if self.done {
            return Ok(None);
        }
        if let Some(bytes) = self.decoded()? {
            return Ok(Some(bytes));
        }
        if self.done {
            return Ok(None);
        }
        let mut bytes = [0; 8192];
        match self.socket.read(&mut bytes) {
            Ok(0) if !self.chunked && self.left.is_none() => {
                self.done = true;
                Ok(None)
            }
            Ok(0) => Err(io::Error::new(io::ErrorKind::UnexpectedEof, "HTTP body")),
            Ok(n) => {
                self.buffer.extend_from_slice(&bytes[..n]);
                Ok(Some(self.decoded()?.unwrap_or_default()))
            }
            Err(e) if timed_out(&e) => Ok(Some(vec![])),
            Err(e) => Err(e),
        }
    }
}
fn timed_out(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted
    )
}
fn get_json(context: &Context, socket: &Path, path: &str) -> io::Result<Value> {
    get_json_until(
        context,
        socket,
        path,
        Instant::now() + Duration::from_secs(2),
    )
}
fn get_json_until(
    context: &Context,
    socket: &Path,
    path: &str,
    deadline: Instant,
) -> io::Result<Value> {
    Http::get(context, socket, path, deadline)?.json(deadline, || context.cancelled())
}

/// A full Unix listen backlog must not hold the observer past its stop deadline.
fn connect(path: &Path) -> io::Result<UnixStream> {
    use std::os::{
        fd::{AsRawFd, FromRawFd},
        unix::ffi::OsStrExt,
    };
    let mut addr: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    let bytes = path.as_os_str().as_bytes();
    if bytes.len() >= addr.sun_path.len() || bytes.contains(&0) {
        return Err(io::Error::other("invalid Unix socket path"));
    }
    addr.sun_family = libc::AF_UNIX as _;
    for (dest, byte) in addr.sun_path.iter_mut().zip(bytes) {
        *dest = *byte as _;
    }
    let fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let socket = unsafe { UnixStream::from_raw_fd(fd) };
    socket.set_nonblocking(true)?;
    if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let length =
        (std::mem::offset_of!(libc::sockaddr_un, sun_path) + bytes.len() + 1) as libc::socklen_t;
    #[cfg(target_os = "macos")]
    {
        addr.sun_len = length as u8;
    }
    if unsafe { libc::connect(fd, (&addr as *const libc::sockaddr_un).cast(), length) } < 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::EINPROGRESS) {
            return Err(error);
        }
        let mut poll = libc::pollfd {
            fd: socket.as_raw_fd(),
            events: libc::POLLOUT,
            revents: 0,
        };
        if unsafe { libc::poll(&mut poll, 1, 500) } <= 0 {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Unix socket connect",
            ));
        }
        if let Some(error) = socket.take_error()? {
            return Err(error);
        }
    }
    socket.set_nonblocking(false)?;
    Ok(socket)
}
