use super::*;
use crate::chats::{Event, Events};
use crate::core::{secrets::Credentials, state::Store};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

struct Request {
    verb: String,
    method: String,
    body: Value,
    authorization: String,
}
struct Reply {
    status: u16,
    body: Value,
    retry: Option<u64>,
}
impl From<Value> for Reply {
    fn from(body: Value) -> Self {
        Self {
            status: 200,
            body,
            retry: None,
        }
    }
}
struct Http {
    url: String,
    stopped: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Http {
    fn new(mut handle: impl FnMut(Request) -> Reply + Send + 'static) -> Self {
        Self::with_optional_reply(move |request| Some(handle(request)))
    }
    fn with_optional_reply(
        mut handle: impl FnMut(Request) -> Option<Reply> + Send + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = stopped.clone();
        let worker = thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    header.push(byte[0]);
                }
                let header = String::from_utf8(header).unwrap();
                let field = |name: &str| {
                    header.lines().find_map(|line| {
                        let (key, value) = line.split_once(':')?;
                        key.eq_ignore_ascii_case(name)
                            .then(|| value.trim().to_owned())
                    })
                };
                let mut body = vec![
                    0;
                    field("content-length")
                        .unwrap_or("0".into())
                        .parse()
                        .unwrap()
                ];
                stream.read_exact(&mut body).unwrap();
                let target = header.split_whitespace().nth(1).unwrap();
                let (method, query) = target.split_once('?').unwrap_or((target, ""));
                let mut params = json!({});
                for pair in query.split('&').filter(|s| !s.is_empty()) {
                    let (key, value) = pair.split_once('=').unwrap();
                    let value = unquote(value);
                    params[key] = if key == "inclusive" {
                        Value::Bool(value == "true")
                    } else {
                        value.into()
                    };
                }
                let request = Request {
                    verb: header.split_whitespace().next().unwrap().into(),
                    method: method.trim_start_matches('/').into(),
                    body: if body.is_empty() {
                        params
                    } else {
                        serde_json::from_slice(&body).unwrap()
                    },
                    authorization: field("authorization").unwrap_or_default(),
                };
                let Some(response) = handle(request) else {
                    continue;
                };
                let body = response.body.to_string();
                write!(stream, "HTTP/1.1 {} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n{}", response.status, body.len(), response.retry.map(|s| format!("Retry-After: {s}\r\n")).unwrap_or_default(), body).unwrap();
            }
        });
        Self {
            url,
            stopped,
            worker: Some(worker),
        }
    }
}
impl Drop for Http {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.url.trim_start_matches("http://").trim_end_matches('/'));
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}
fn options(events: Events) -> Connect {
    Connect {
        config: ChatConfig {
            kind: "slack".into(),
            owner: "UOWNER".into(),
            channels: vec![],
        },
        proxy: None,
        credentials: Arc::new(|| {
            Ok(Credentials(BTreeMap::from([
                ("app_token", "xapp-test".into()),
                ("bot_token", "xoxb-test".into()),
            ])))
        }),
        events,
    }
}
fn unquote(value: &str) -> String {
    let mut bytes = Vec::new();
    let mut value = value.as_bytes();
    while let Some((&byte, rest)) = value.split_first() {
        if byte == b'%' {
            bytes.push(u8::from_str_radix(std::str::from_utf8(&rest[..2]).unwrap(), 16).unwrap());
            value = &rest[2..];
        } else {
            bytes.push(if byte == b'+' { b' ' } else { byte });
            value = rest;
        }
    }
    String::from_utf8(bytes).unwrap()
}
fn auth() -> Value {
    json!({"ok":true,"team":"Test workspace","user":"pony","user_id":"UBOT","bot_id":"BBOT"})
}
fn socket() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!(
        "ws://{}/?ticket=test-ticket",
        listener.local_addr().unwrap()
    );
    (listener, url)
}
fn accept(listener: &TcpListener) -> tungstenite::WebSocket<TcpStream> {
    let (stream, _) = listener.accept().unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    tungstenite::accept(stream).unwrap()
}
fn write(ws: &mut tungstenite::WebSocket<TcpStream>, value: Value) {
    ws.send(tungstenite::Message::Text(value.to_string().into()))
        .unwrap();
}
#[test]
fn connects_reports_metadata_and_persists_before_ack() {
    let (listener, url) = socket();
    let http = Http::new(move |r| match r.method.as_str() {
        "apps.connections.open" => {
            assert_eq!(r.authorization, "Bearer xapp-test");
            json!({"ok":true,"url":url}).into()
        }
        "auth.test" => {
            assert_eq!(r.authorization, "Bearer xoxb-test");
            auth().into()
        }
        _ => panic!("unexpected method"),
    });
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let durable = store.events("slack", "UOWNER");
    let (tx, rx) = mpsc::channel();
    let events = Events(Arc::new(move |e| {
        durable.emit(e.clone())?;
        tx.send(e).unwrap();
        Ok(())
    }));
    let mut connection = connect_at(options(events), &http.url).unwrap();
    // RED: the old placeholder never emits a connection event.
    assert!(rx.recv_timeout(Duration::from_millis(500)).is_ok());
    let worker = thread::spawn(move || {
        let mut ws = accept(&listener);
        write(&mut ws, json!({"type":"hello","num_connections":3}));
        write(
            &mut ws,
            json!({"type":"events_api","envelope_id":"e1","payload":{"event":{
                "type":"message","channel":"C1","user":"UOWNER","ts":"123.001","text":"<@UBOT> hi &lt;x&gt; &amp; <https://test.invalid|test>"
            }}}),
        );
        let ack: Value = serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(ack, json!({"envelope_id":"e1"}));
        let disk: Value =
            serde_json::from_slice(&std::fs::read(dir.path().join("state.json")).unwrap()).unwrap();
        let encoded = disk.to_string();
        assert!(encoded.contains("hi <x> & https://test.invalid"));
        assert!(!encoded.contains("xapp-test"));
        assert!(!encoded.contains("test-ticket"));
    });
    let connected = loop {
        if let Event::Connection(info) = rx.recv_timeout(Duration::from_secs(3)).unwrap()
            && info.connection == "connected"
        {
            break info;
        }
    };
    assert_eq!(connected.workspace.as_deref(), Some("Test workspace"));
    assert_eq!(connected.bot.as_deref(), Some("pony"));
    assert_eq!(connected.other_connections, 2);
    let Event::Message(message) = rx.recv_timeout(Duration::from_secs(3)).unwrap() else {
        panic!("message expected")
    };
    assert!(message.mentioned);
    assert_eq!(message.text, "hi <x> & https://test.invalid");
    worker.join().unwrap();
    connection.close();
}

fn http_connection(http: &Http) -> SlackConnection {
    let mut api = Api::new(None).unwrap();
    api.base = http.url.clone();
    SlackConnection {
        api,
        session: Arc::new(Mutex::new(Session {
            token: "xoxb-test".into(),
            user: "UBOT".into(),
            bot_id: "BBOT".into(),
            ..Session::default()
        })),
        worker: None,
        owner: "UOWNER".into(),
        sent: BTreeMap::new(),
        problems: BTreeMap::new(),
    }
}
#[test]
fn sends_markdown_threads_safe_mentions_and_retries_same_body_after_429() {
    let (tx, rx) = mpsc::channel();
    let mut count = 0;
    let http = Http::new(move |r| {
        assert_eq!(r.method, "chat.postMessage");
        assert_eq!(r.authorization, "Bearer xoxb-test");
        count += 1;
        tx.send((Instant::now(), r.body)).unwrap();
        if count == 1 {
            Reply {
                status: 429,
                body: json!({"secret":"must not escape"}),
                retry: Some(1),
            }
        } else {
            json!({"ok":true,"ts":format!("{count}.001")}).into()
        }
    });
    let mut connection = http_connection(&http);
    let markdown = "body <!channel> <!here> <!everyone> <@UOTHER> <!subteam^S1> &lt;@UOTHER&gt;";
    let outgoing = || Outgoing {
        channel: "C1",
        thread: Some("1.001"),
        markdown,
        notify: true,
    };
    assert_eq!(connection.send(outgoing()).unwrap(), "2.001");
    assert_eq!(connection.send(outgoing()).unwrap(), "3.001");
    let (first, body) = rx.recv().unwrap();
    let (retry, retried) = rx.recv().unwrap();
    let (next, _) = rx.recv().unwrap();
    assert!(retry.duration_since(first) >= Duration::from_secs(1));
    assert!(next.duration_since(retry) >= Duration::from_secs(1));
    assert_eq!(body, retried);
    assert_eq!(body["thread_ts"], "1.001");
    assert_eq!(body["unfurl_links"], false);
    assert_eq!(body["unfurl_media"], false);
    assert_eq!(body["blocks"][0]["type"], "section");
    assert!(
        body["blocks"][0]["text"]["text"]
            .as_str()
            .unwrap()
            .starts_with("<@UOWNER> ")
    );
    assert_eq!(body["blocks"][1]["type"], "markdown");
    let rendered = body["blocks"][1]["text"].as_str().unwrap();
    assert!(!rendered.contains("<!"));
    assert!(!rendered.contains("<@"));
    assert!(rendered.contains("&lt;@UOTHER&gt;"));
    assert!(rendered.contains("&amp;lt;@UOTHER&amp;gt;"));
    assert!(!body["text"].as_str().unwrap().contains('\n'));
}

#[test]
fn channel_metadata_and_errors_are_mapped_without_raw_error_text() {
    let http = Http::new(|r| {
        if r.verb != "GET" {
            return json!({"ok":false,"error":"wrong_http_method"}).into();
        }
        let id = r.body["channel"].as_str().unwrap();
        match id {
            "CMEMBER" => {
                json!({"ok":true,"channel":{"name":"room","is_private":true,"is_member":true}})
            }
            "COUT" => json!({"ok":true,"channel":{"is_member":false}}),
            "COLD" => json!({"ok":true,"channel":{"is_archived":true}}),
            "CUNKNOWN" => json!({"ok":true,"channel":{"name":"unknown"}}),
            "CMISSING" => json!({"ok":false,"error":"channel_not_found"}),
            _ => json!({"ok":false,"error":"xoxb-test Authorization wss://ticket"}),
        }
        .into()
    });
    let mut c = http_connection(&http);
    let info = c.channel("CMEMBER").unwrap();
    assert_eq!(info.name.as_deref(), Some("room"));
    assert_eq!(info.private, Some(true));
    assert!(info.problem.is_none());
    for (id, expected) in [
        ("COUT", "not_member"),
        ("COLD", "archived"),
        ("CMISSING", "not_found"),
    ] {
        assert_eq!(c.channel(id).unwrap().problem.as_deref(), Some(expected));
    }
    assert!(c.channel("CUNKNOWN").unwrap().problem.is_none());
    let error = c.channel("CBAD").unwrap_err();
    assert_eq!(error.code, "slack_error");
    assert!(!error.message.contains("xoxb-test"));
}

#[test]
fn history_paginates_and_reads_only_threads_with_new_replies() {
    let bound = now() - 100.;
    let parent = format!("{:.6}", bound - 100.);
    let parent_id = parent.clone();
    let http = Http::new(move |r| {
        let cursor = r.body["cursor"].as_str().unwrap_or_default();
        match (r.method.as_str(), cursor) {
            ("conversations.history", "") => {
                assert!(r.body["oldest"].as_str().unwrap().parse::<f64>().unwrap() <= bound - 29. * 86400.);
                json!({"ok":true,"messages":[
                    {"ts":format!("{:.6}",bound+1.),"user":"UOWNER","text":"new"},
                    {"ts":parent_id,"user":"UOWNER","text":"old","latest_reply":format!("{:.6}",bound+3.)}
                ],"has_more":true,"response_metadata":{"next_cursor":"history-2"}}).into()
            }
            ("conversations.history", "history-2") => json!({"ok":true,"messages":[
                {"ts":format!("{:.6}",bound-10.),"user":"UOWNER","text":"unchanged","latest_reply":format!("{:.6}",bound-1.)},
                {"ts":format!("{:.6}",bound-20.),"user":"UOWNER","text":"no replies"}
            ],"has_more":false}).into(),
            ("conversations.replies", "") => {
                assert_eq!(r.body["ts"], parent_id);
                assert_eq!(r.body["oldest"], format!("{bound:.6}"));
                json!({"ok":true,"messages":[
                    {"ts":parent_id,"user":"UOWNER","text":"parent repeated"},
                    {"ts":format!("{:.6}",bound+2.),"thread_ts":parent_id,"user":"UOWNER","text":"<@UBOT> reply &amp;","subtype":"file_share","files":[{}]}
                ],"response_metadata":{"next_cursor":"reply-2"}}).into()
            }
            ("conversations.replies", "reply-2") => json!({"ok":true,"messages":[
                {"ts":format!("{:.6}",bound+3.),"thread_ts":parent_id,"user":"UOWNER","text":"last"},
                {"ts":format!("{:.6}",bound+4.),"bot_id":"B1","user":"UBOT","text":"bot"},
                {"ts":format!("{:.6}",bound+5.),"user":"UOWNER","subtype":"message_changed","text":"edit"}
            ],"has_more":false}).into(),
            _ => panic!("unexpected history request"),
        }
    });
    let mut c = http_connection(&http);
    let messages = c
        .history(
            "C1",
            &HistoryAfter {
                id: Some(format!("{bound:.6}")),
                at: bound,
            },
        )
        .unwrap();
    assert_eq!(
        messages.iter().map(|m| m.text.as_str()).collect::<Vec<_>>(),
        ["new", "reply &", "last"]
    );
    assert_eq!(messages[1].thread.as_deref(), Some(parent.as_str()));
    assert!(messages[1].mentioned && messages[1].attachments);
    assert!(messages.iter().all(|m| m.channel == "C1"));
}

#[test]
fn exists_distinguishes_deletion_from_inaccessible_or_incomplete_history() {
    let http = Http::new(|r| {
        assert_eq!(r.body["inclusive"], true);
        match r.body["channel"].as_str().unwrap() {
            "CPRESENT" => {
                assert_eq!(r.method, "conversations.history");
                json!({"ok":true,"messages":[{"ts":"123.001"}]}).into()
            }
            "CGONE" => json!({"ok":true,"messages":[]}).into(),
            "CTHREAD" => {
                assert_eq!(r.method, "conversations.replies");
                assert_eq!(r.body["ts"], "100.001");
                if r.body["cursor"].is_null() {
                    json!({"ok":true,"messages":[{"ts":"100.001"}],"has_more":true,"response_metadata":{"next_cursor":"p2"}}).into()
                } else {
                    json!({"ok":true,"messages":[{"ts":"123.001"}]}).into()
                }
            }
            "CDENIED" => json!({"ok":false,"error":"not_in_channel"}).into(),
            "CBROKEN" => json!({"ok":true,"messages":[],"has_more":true}).into(),
            _ => Reply {
                status: 503,
                body: Value::Null,
                retry: None,
            },
        }
    });
    let mut c = http_connection(&http);
    assert!(c.exists("CPRESENT", None, "123.001").unwrap());
    assert!(!c.exists("CGONE", None, "123.001").unwrap());
    assert!(c.exists("CTHREAD", Some("100.001"), "123.001").unwrap());
    for channel in ["CDENIED", "CBROKEN", "CNETWORK"] {
        assert!(c.exists(channel, None, "123.001").is_err());
    }
}

fn events() -> (Events, mpsc::Receiver<Event>) {
    let (tx, rx) = mpsc::channel();
    (
        Events(Arc::new(move |e| {
            let _ = tx.send(e);
            Ok(())
        })),
        rx,
    )
}
fn wait_connection(
    rx: &mpsc::Receiver<Event>,
    state: &str,
    reason: Option<&str>,
) -> ConnectionInfo {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Event::Connection(info) = rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap()
            && info.connection == state
            && info.reason.as_deref() == reason
        {
            return info;
        }
    }
}

#[test]
fn socket_receives_and_acks_while_send_waits_for_rate_limit() {
    let (listener, url) = socket();
    let (limited, wait_limited) = mpsc::channel();
    let (acknowledged, wait_ack) = mpsc::channel();
    let mut posted = 0;
    let http = Http::new(move |r| match r.method.as_str() {
        "apps.connections.open" => json!({"ok":true,"url":url}).into(),
        "auth.test" => auth().into(),
        "chat.postMessage" => {
            posted += 1;
            if posted == 1 {
                limited.send(()).unwrap();
                Reply {
                    status: 429,
                    body: Value::Null,
                    retry: Some(1),
                }
            } else {
                assert!(
                    wait_ack.try_recv().is_ok(),
                    "ACK must arrive during the send wait"
                );
                json!({"ok":true,"ts":"456.001"}).into()
            }
        }
        _ => panic!("unexpected method"),
    });
    let worker = thread::spawn(move || {
        let mut ws = accept(&listener);
        write(&mut ws, json!({"type":"hello"}));
        wait_limited.recv_timeout(Duration::from_secs(3)).unwrap();
        ws.send(Frame::Ping(vec![7].into())).unwrap();
        write(
            &mut ws,
            json!({"type":"events_api","envelope_id":"during-limit","payload":{"event":{
                "type":"message","channel":"C1","user":"UOWNER","ts":"124.001","thread_ts":"123.001","subtype":"file_share","text":"text","files":[{}]
            }}}),
        );
        assert_eq!(ws.read().unwrap(), Frame::Pong(vec![7].into()));
        assert!(
            ws.read()
                .unwrap()
                .to_text()
                .unwrap()
                .contains("during-limit")
        );
        acknowledged.send(()).unwrap();
    });
    let (events, rx) = events();
    let mut c = connect_at(options(events), &http.url).unwrap();
    wait_connection(&rx, "connected", None);
    assert_eq!(
        c.send(Outgoing {
            channel: "C1",
            thread: None,
            markdown: "hi",
            notify: false
        })
        .unwrap(),
        "456.001"
    );
    let Event::Message(message) = rx.recv_timeout(Duration::from_secs(2)).unwrap() else {
        panic!("message expected")
    };
    assert_eq!(message.thread.as_deref(), Some("123.001"));
    assert!(message.attachments);
    c.close();
    worker.join().unwrap();
}

#[test]
fn only_other_human_messages_in_bound_channels_are_logged_as_ignored() {
    use crate::core::{config::Channel, engine::Engine};
    use crate::corral;

    struct NoCorral;
    impl corral::Corral for NoCorral {
        fn list(&mut self) -> corral::Result<Vec<corral::Agent>> {
            panic!("ignored events must not call corral")
        }
        fn status(&mut self, _: &str) -> corral::Result<corral::Status> {
            panic!("ignored events must not call corral")
        }
        fn reply(&mut self, _: &str) -> corral::Result<corral::Reply> {
            panic!("ignored events must not call corral")
        }
        fn send(&mut self, _: &str, _: &str) -> corral::Result<corral::Sent> {
            panic!("ignored events must not call corral")
        }
    }

    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let mut options = options(store.events("slack", "UOWNER"));
    options.config.channels.push(Channel {
        id: "C1".into(),
        repo: "test".into(),
    });
    let mut engine = Engine::new(store, vec![options.config.clone()], BTreeMap::new());
    let session = Session {
        user: "UBOT".into(),
        bot_id: "BBOT".into(),
        ..Session::default()
    };
    let mut logged = Vec::new();
    for event in [
        json!({"type":"message","channel":"C1","user":"UBOT","bot_id":"BBOT","ts":"1.001","text":"echo"}),
        json!({"type":"message","channel":"C1","user":"UBOT","ts":"2.001","text":"echo"}),
        json!({"type":"message","channel":"C1","user":"UOTHERBOT","bot_id":"BOTHER","ts":"3.001","text":"bot"}),
        json!({"type":"message","channel":"C1","user":"UOTHER","subtype":"message_changed","ts":"4.001"}),
        json!({"type":"message","channel":"C1","user":"UOTHER","subtype":"channel_join","ts":"5.001"}),
        json!({"type":"message","channel":"C2","user":"UOTHER","ts":"6.001","text":"unbound"}),
        json!({"type":"message","channel":"C1","user":"UOTHER","ts":"7.001","text":"private text"}),
    ] {
        emit_received(
            &json!({"type":"events_api","payload":{"event":event}}),
            &options,
            &session,
        )
        .unwrap();
        engine.step(&mut NoCorral, 10.).unwrap();
        logged.push(engine.log.iter().any(|line| line == "忽略了一条别人的消息"));
        engine.log.clear();
    }
    assert_eq!(logged, [false, false, false, false, false, false, true]);
}

#[test]
fn filters_events_and_does_not_ack_when_emit_fails() {
    let (listener, url) = socket();
    let http = Http::new(move |r| {
        if r.method == "auth.test" {
            auth().into()
        } else {
            json!({"ok":true,"url":url}).into()
        }
    });
    let (events, rx) = events();
    let events = Events(Arc::new(move |event| {
        if matches!(&event, Event::Message(m) if m.id == "999.001") {
            return Err(error("io"));
        }
        events.emit(event)
    }));
    let worker = thread::spawn(move || {
        let mut ws = accept(&listener);
        write(&mut ws, json!({"type":"hello"}));
        // Duplicate hello must not trigger another connected/backfill event.
        write(&mut ws, json!({"type":"hello"}));
        for (envelope, event) in [
            (
                "root",
                json!({"type":"message","channel":"C1","user":"UOWNER","ts":"1.001","thread_ts":"1.001","text":"normal"}),
            ),
            (
                "bot",
                json!({"type":"message","channel":"C1","user":"UOWNER","bot_id":"B1","ts":"2.001","text":"ignore"}),
            ),
            (
                "edit",
                json!({"type":"message","channel":"C1","user":"UOWNER","subtype":"message_changed","ts":"3.001"}),
            ),
            (
                "delete",
                json!({"type":"message","channel":"C1","subtype":"message_deleted","hidden":true,"deleted_ts":"1.001"}),
            ),
        ] {
            write(
                &mut ws,
                json!({"type":"events_api","envelope_id":envelope,"payload":{"event":event}}),
            );
            assert!(ws.read().unwrap().to_text().unwrap().contains(envelope));
        }
        write(
            &mut ws,
            json!({"type":"events_api","envelope_id":"fail","payload":{"event":{
                "type":"message","channel":"C1","user":"UOWNER","ts":"999.001","text":"not durable"
            }}}),
        );
        assert!(
            ws.read().is_err(),
            "failed persistence must close without an ACK"
        );
    });
    let mut c = connect_at(options(events), &http.url).unwrap();
    wait_connection(&rx, "connected", None);
    let Event::Message(root) = rx.recv_timeout(Duration::from_secs(3)).unwrap() else {
        panic!("root")
    };
    assert!(root.thread.is_none());
    // Bot messages and edits are ACKed above but emit no event to the core.
    assert!(
        matches!(rx.recv_timeout(Duration::from_secs(3)).unwrap(), Event::Deleted { channel, id } if channel == "C1" && id == "1.001")
    );
    worker.join().unwrap();
    c.close();
}

#[test]
fn reconnects_with_fresh_credentials_and_backs_off_short_connections() {
    let (listener, url) = socket();
    let (request_tx, requests) = mpsc::channel();
    let http = Http::new(move |r| {
        if r.method == "auth.test" {
            return auth().into();
        }
        request_tx.send((Instant::now(), r.authorization)).unwrap();
        json!({"ok":true,"url":url}).into()
    });
    let (events, rx) = events();
    let mut opts = options(events);
    let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = reads.clone();
    opts.credentials = Arc::new(move || {
        let n = count.fetch_add(1, Ordering::Relaxed);
        Ok(Credentials(BTreeMap::from([
            ("app_token", format!("xapp-test-{n}")),
            ("bot_token", "xoxb-test".into()),
        ])))
    });
    let worker = thread::spawn(move || {
        for reason in [
            Some("warning"),
            Some("refresh_requested"),
            None,
            Some("link_disabled"),
        ] {
            let mut ws = accept(&listener);
            write(&mut ws, json!({"type":"hello"}));
            if let Some(reason) = reason {
                write(&mut ws, json!({"type":"disconnect","reason":reason}));
            }
            // None drops the TCP connection to simulate a network failure.
        }
    });
    let mut c = connect_at(opts, &http.url).unwrap();
    let first = requests.recv_timeout(Duration::from_secs(3)).unwrap();
    let second = requests.recv_timeout(Duration::from_secs(3)).unwrap();
    let third = requests.recv_timeout(Duration::from_secs(4)).unwrap();
    let fourth = requests.recv_timeout(Duration::from_secs(6)).unwrap();
    assert!(second.0.duration_since(first.0) >= Duration::from_secs(1));
    assert!(third.0.duration_since(second.0) >= Duration::from_secs(2));
    assert!(fourth.0.duration_since(third.0) >= Duration::from_secs(4));
    assert_eq!(
        [first.1, second.1, third.1, fourth.1],
        [
            "Bearer xapp-test-0",
            "Bearer xapp-test-1",
            "Bearer xapp-test-2",
            "Bearer xapp-test-3"
        ]
    );
    wait_connection(&rx, "failed", Some("socket_mode_off"));
    c.close();
    worker.join().unwrap();
    assert_eq!(reads.load(Ordering::Relaxed), 4);
}

#[test]
fn authentication_rate_limit_and_secret_errors_have_public_status() {
    for (response, expected) in [
        (401, "auth_failed"),
        (429, "rate_limited"),
        (200, "auth_failed"),
    ] {
        let http = Http::new(move |_| Reply {
            status: response,
            body: json!({"ok":false,"error":"invalid_auth","private":"xoxb-test ticket"}),
            retry: Some(60),
        });
        let (events, rx) = events();
        let mut c = connect_at(options(events), &http.url).unwrap();
        let info = wait_connection(&rx, "failed", Some(expected));
        assert!(!serde_json::to_string(&info).unwrap().contains("xoxb-test"));
        let start = Instant::now();
        c.close();
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "closing interrupts retry waits"
        );
    }
    let http = Http::new(|_| panic!("invalid secrets must prevent all HTTP"));
    let (events, rx) = events();
    let mut opts = options(events);
    opts.credentials = Arc::new(|| Err(Error::new("secrets_too_open", "private path xoxb-test")));
    let mut c = connect_at(opts, &http.url).unwrap();
    let info = wait_connection(&rx, "failed", Some("secrets_too_open"));
    assert!(!info.message.unwrap().contains("private path"));
    c.close();
}

#[test]
fn heartbeat_sends_ping_and_detects_a_silent_peer() {
    let (listener, url) = socket();
    let http = Http::new(move |r| {
        if r.method == "auth.test" {
            auth().into()
        } else {
            json!({"ok":true,"url":url}).into()
        }
    });
    let (events, rx) = events();
    let mut api = Api::new(None).unwrap();
    api.base = http.url.clone();
    api.heartbeat = Heartbeat {
        ping: 0.05,
        dead: 0.7,
    };
    let worker = thread::spawn(move || {
        let mut ws = accept(&listener);
        write(&mut ws, json!({"type":"hello"}));
        assert!(matches!(ws.read().unwrap(), Frame::Ping(_)));
        // Don't read/flush again: tungstenite would automatically answer the ping.
        thread::sleep(Duration::from_secs(1));
    });
    let mut c = start(options(events), api).unwrap();
    wait_connection(&rx, "connected", None);
    wait_connection(&rx, "failed", Some("network"));
    c.close();
    worker.join().unwrap();
}

#[test]
fn websocket_uses_configured_connect_proxy() {
    let (listener, url) = socket();
    let proxy = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = proxy.local_addr().unwrap();
    let peer = thread::spawn(move || {
        let mut ws = accept(&listener);
        write(&mut ws, json!({"type":"hello"}));
    });
    let worker = thread::spawn(move || {
        let (mut client, _) = proxy.accept().unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut header = Vec::new();
        while !header.ends_with(b"\r\n\r\n") {
            let mut b = [0];
            client.read_exact(&mut b).unwrap();
            header.push(b[0]);
        }
        let header = String::from_utf8(header).unwrap();
        assert!(header.starts_with("CONNECT 127.0.0.1:"));
        let target = header.split_whitespace().nth(1).unwrap();
        let mut upstream = TcpStream::connect(target).unwrap();
        client
            .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
            .unwrap();
        let mut outgoing = upstream.try_clone().unwrap();
        let mut incoming = client.try_clone().unwrap();
        let forward = thread::spawn(move || {
            let _ = std::io::copy(&mut incoming, &mut outgoing);
            let _ = outgoing.shutdown(std::net::Shutdown::Write);
        });
        let _ = std::io::copy(&mut upstream, &mut client);
        let _ = client.shutdown(std::net::Shutdown::Write);
        forward.join().unwrap();
    });
    let api = Api::new(Some(&format!("http://{address}"))).unwrap();
    let mut ws = open_socket(&api, &url).unwrap();
    assert!(ws.read().unwrap().to_text().unwrap().contains("hello"));
    drop(ws);
    peer.join().unwrap();
    worker.join().unwrap();
}

#[test]
fn size_errors_request_resize_and_membership_errors_are_remembered() {
    let http = Http::new(|r| {
        if r.method == "conversations.info" {
            return json!({"ok":true,"channel":{"name":"no-member-field"}}).into();
        }
        let raw = match r.body["channel"].as_str().unwrap() {
            "CLONG" => "msg_too_long",
            "CBLOCK" => "invalid_blocks",
            "COUT" => "not_in_channel",
            _ => "some xoxb-test private body",
        };
        json!({"ok":false,"error":raw}).into()
    });
    let mut c = http_connection(&http);
    for (channel, code, resize) in [
        ("CLONG", "message_too_long", true),
        ("CBLOCK", "invalid_blocks", true),
        ("COUT", "not_member", false),
        ("COTHER", "slack_error", false),
    ] {
        let e = c
            .send(Outgoing {
                channel,
                thread: None,
                markdown: "body",
                notify: false,
            })
            .unwrap_err();
        assert_eq!(e.code, code);
        assert_eq!(e.resize, resize);
    }
    assert_eq!(
        c.channel("COUT").unwrap().problem.as_deref(),
        Some("not_member")
    );
}

#[test]
fn socket_recovers_after_a_connection_report_cannot_be_persisted() {
    for failing_state in ["connecting", "failed"] {
        let (listener, url) = socket();
        let http = Http::new(move |r| {
            if r.method == "auth.test" {
                auth().into()
            } else {
                json!({"ok":true,"url":url}).into()
            }
        });
        let (events, rx) = events();
        let failed_once = Arc::new(AtomicBool::new(false));
        let failed = failed_once.clone();
        let events = Events(Arc::new(move |event| {
            if matches!(&event, Event::Connection(info) if info.connection == failing_state)
                && !failed.swap(true, Ordering::Relaxed)
            {
                return Err(error("io"));
            }
            events.emit(event)
        }));
        let peer = thread::spawn(move || {
            let mut first = accept(&listener);
            write(&mut first, json!({"type":"hello"}));
            drop(first);
            let mut second = accept(&listener);
            write(&mut second, json!({"type":"hello"}));
            write(
                &mut second,
                json!({"type":"events_api","envelope_id":"recovered","payload":{"event":{
                    "type":"message","channel":"C1","user":"UOWNER","ts":"123.001","text":"after recovery"
                }}}),
            );
            assert!(
                second
                    .read()
                    .unwrap()
                    .to_text()
                    .unwrap()
                    .contains("recovered")
            );
        });
        let mut c = connect_at(options(events), &http.url).unwrap();
        wait_connection(&rx, "connected", None);
        wait_connection(&rx, "connected", None);
        let Event::Message(message) = rx.recv_timeout(Duration::from_secs(3)).unwrap() else {
            panic!("message after recovery");
        };
        assert_eq!(message.text, "after recovery");
        assert!(failed_once.load(Ordering::Relaxed));
        peer.join().unwrap();
        c.close();
    }
}

#[test]
fn oversized_markdown_blocks_request_resize() {
    let http = Http::new(|_| json!({"ok":false,"error":"msg_blocks_too_long"}).into());
    let mut c = http_connection(&http);
    let e = c
        .send(Outgoing {
            channel: "C1",
            thread: None,
            markdown: "<>&",
            notify: false,
        })
        .unwrap_err();
    assert_eq!(e.code, "invalid_blocks");
    assert!(e.resize);
}

#[test]
fn broadcast_thread_reply_is_delivered_live_and_deduplicated_in_history() {
    let (listener, url) = socket();
    let at = now() - 5.;
    let parent = format!("{:.6}", at - 10.);
    let message = json!({"type":"message","subtype":"thread_broadcast","channel":"C1",
        "user":"UOWNER","ts":format!("{at:.6}"),"thread_ts":parent,"text":"<@UBOT> continue"});
    let history_message = message.clone();
    let http = Http::new(move |r| {
        match r.method.as_str() {
        "apps.connections.open" => json!({"ok":true,"url":url}).into(),
        "auth.test" => auth().into(),
        "conversations.history" => json!({"ok":true,"messages":[history_message,
            {"ts":history_message["thread_ts"],"user":"UOWNER","text":"parent","latest_reply":history_message["ts"]}
        ]}).into(),
        "conversations.replies" => json!({"ok":true,"messages":[history_message]}).into(),
        _ => panic!("unexpected method"),
    }
    });
    let peer = thread::spawn(move || {
        let mut ws = accept(&listener);
        write(&mut ws, json!({"type":"hello"}));
        write(
            &mut ws,
            json!({"type":"events_api","envelope_id":"broadcast","payload":{"event":message}}),
        );
        assert!(ws.read().unwrap().to_text().unwrap().contains("broadcast"));
    });
    let (events, rx) = events();
    let mut c = connect_at(options(events), &http.url).unwrap();
    wait_connection(&rx, "connected", None);
    let Event::Message(live) = rx.recv_timeout(Duration::from_secs(3)).unwrap() else {
        panic!("broadcast must be a message");
    };
    assert_eq!(live.thread.as_deref(), Some(parent.as_str()));
    assert_eq!(live.text, "continue");
    assert!(live.mentioned);
    let found = c
        .history(
            "C1",
            &HistoryAfter {
                id: None,
                at: at - 1.,
            },
        )
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(
        serde_json::to_value(&found[0]).unwrap(),
        serde_json::to_value(live).unwrap()
    );
    peer.join().unwrap();
    c.close();
}

#[test]
fn bot_id_mention_is_recognized_and_removed() {
    let value = json!({"type":"message","channel":"C1","user":"UOWNER",
        "ts":"123.001","text":"<@BBOT> 手机测试"});
    let Some(Event::Message(message)) = translate(&value, "UBOT", "BBOT") else {
        panic!("message expected");
    };
    assert!(message.mentioned);
    assert_eq!(message.text, "手机测试");
}

#[test]
fn shared_messages_are_reported_as_omitted_attachments() {
    for link in ["from_url", "original_url"] {
        let mut attachment = json!({"is_share":true,"text":"forwarded content"});
        attachment[link] = "https://test.invalid/message".into();
        let mut value = json!({"type":"message","channel":"C1","user":"UOWNER",
            "ts":"123.001","text":"<@UBOT> look at this","attachments":[attachment]});
        let Some(Event::Message(message)) = translate(&value, "UBOT", "BBOT") else {
            panic!("shared message must be delivered");
        };
        assert!(
            message.attachments,
            "a share must not be treated as a preview"
        );
        assert_eq!(message.text, "look at this");
        assert!(message.mentioned);

        value["attachments"][0]["is_share"] = false.into();
        let Some(Event::Message(preview)) = translate(&value, "UBOT", "BBOT") else {
            panic!("link message must be delivered");
        };
        assert!(!preview.attachments, "pasted links still have previews");
    }
}

#[test]
fn link_previews_in_history_are_not_reported_as_omitted_attachments() {
    let at = now() - 10.;
    let http = Http::new(move |_| {
        let variants = [
            json!({"attachments":[{"from_url":"https://test.invalid"}]}),
            json!({"attachments":[{"original_url":"https://test.invalid"}]}),
            json!({"attachments":[{"from_url":"https://test.invalid","original_url":"https://test.invalid"}]}),
            json!({"attachments":[{"from_url":"https://test.invalid"},{"fallback":"actual attachment"}]}),
            json!({"files":[{"id":"F1"}]}),
            json!({"subtype":"file_share"}),
        ];
        let messages: Vec<_> = variants
            .into_iter()
            .enumerate()
            .map(|(i, mut m)| {
                m["ts"] = format!("{:.6}", at + i as f64).into();
                m["user"] = "UOWNER".into();
                m["text"] = "<https://test.invalid|link>".into();
                m
            })
            .collect();
        json!({"ok":true,"messages":messages}).into()
    });
    let mut c = http_connection(&http);
    let found = c
        .history(
            "C1",
            &HistoryAfter {
                id: None,
                at: at - 1.,
            },
        )
        .unwrap();
    assert_eq!(
        found.iter().map(|m| m.attachments).collect::<Vec<_>>(),
        [false, false, false, true, true, true]
    );
    assert!(found.iter().all(|m| m.text == "https://test.invalid"));
}

#[test]
fn send_retries_only_unsent_requests_and_close_interrupts_backoff() {
    use std::{collections::VecDeque, sync::atomic::AtomicUsize};
    #[derive(Debug)]
    struct FailSetup {
        attempts: Arc<Mutex<Vec<Instant>>>,
        errors: Mutex<VecDeque<ureq::Error>>,
        attempted: Option<mpsc::Sender<()>>,
    }
    impl Connector for FailSetup {
        type Out = Box<dyn Transport>;
        fn connect(
            &self,
            details: &ConnectionDetails<'_>,
            chained: Option<()>,
        ) -> std::result::Result<Option<Self::Out>, ureq::Error> {
            self.attempts.lock().unwrap().push(Instant::now());
            if let Some(tx) = &self.attempted {
                let _ = tx.send(());
            }
            if let Some(e) = self.errors.lock().unwrap().pop_front() {
                return Err(e);
            }
            HttpConnector::default().connect(details, chained)
        }
    }
    let received = Arc::new(AtomicUsize::new(0));
    let count = received.clone();
    let http = Http::new(move |request| {
        assert_eq!(request.method, "chat.postMessage");
        assert_eq!(request.body["blocks"][0]["text"], "send once");
        count.fetch_add(1, Ordering::Relaxed);
        json!({"ok":true,"ts":"123.001"}).into()
    });
    let mut c = http_connection(&http);
    let attempts = Arc::new(Mutex::new(Vec::new()));
    c.api.agent = ureq::Agent::with_parts(
        c.api.agent.config().clone(),
        FailSetup {
            attempts: attempts.clone(),
            attempted: None,
            errors: Mutex::new(VecDeque::from([
                ureq::Error::HostNotFound,
                ureq::Error::Timeout(ureq::Timeout::Connect),
                ureq::Error::ConnectProxyFailed("test CONNECT failure".into()),
            ])),
        },
        DefaultResolver::default(),
    );
    assert_eq!(
        c.send(Outgoing {
            channel: "C1",
            thread: None,
            markdown: "send once",
            notify: false
        })
        .unwrap(),
        "123.001"
    );
    assert_eq!(received.load(Ordering::Relaxed), 1);
    let times = attempts.lock().unwrap();
    assert_eq!(times.len(), 4);
    for (pair, seconds) in times.windows(2).zip([1, 2, 4]) {
        assert!(pair[1].duration_since(pair[0]) >= Duration::from_secs(seconds));
    }
    drop(times);

    // The same low-level I/O kind is retryable only when the connector produced it.
    for kind in [
        std::io::ErrorKind::ConnectionRefused,
        std::io::ErrorKind::NetworkUnreachable,
        std::io::ErrorKind::HostUnreachable,
    ] {
        assert!(unsent(&connection_error(ureq::Error::Io(kind.into()))));
        assert!(!unsent(&ureq::Error::Io(kind.into())));
    }
    assert!(unsent(&ureq::Error::Timeout(ureq::Timeout::Resolve)));
    for phase in [
        ureq::Timeout::Global,
        ureq::Timeout::SendRequest,
        ureq::Timeout::SendBody,
        ureq::Timeout::RecvResponse,
        ureq::Timeout::RecvBody,
    ] {
        assert!(!unsent(&ureq::Error::Timeout(phase)));
    }

    // Slack accepted the whole request, but its response was lost: never replay it.
    let received = Arc::new(AtomicUsize::new(0));
    let count = received.clone();
    let lost_response = Http::with_optional_reply(move |_| {
        count.fetch_add(1, Ordering::Relaxed);
        None
    });
    let mut ambiguous = http_connection(&lost_response);
    let e = ambiguous
        .send(Outgoing {
            channel: "C1",
            thread: None,
            markdown: "do not repeat",
            notify: false,
        })
        .unwrap_err();
    assert_eq!(e.code, "network");
    assert!(!e.resize);
    assert_eq!(received.load(Ordering::Relaxed), 1);

    let (tx, rx) = mpsc::channel();
    c.api.agent = ureq::Agent::with_parts(
        c.api.agent.config().clone(),
        FailSetup {
            attempts: Arc::new(Mutex::new(Vec::new())),
            attempted: Some(tx),
            errors: Mutex::new(VecDeque::from([ureq::Error::ConnectionFailed])),
        },
        DefaultResolver::default(),
    );
    let api = c.api.clone();
    let waiting = thread::spawn(move || api.call("chat.postMessage", "xoxb-test", json!({}), None));
    rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let closing = Instant::now();
    c.close();
    assert_eq!(waiting.join().unwrap().unwrap_err().code, "stopped");
    assert!(closing.elapsed() < Duration::from_millis(500));
}
