use super::{
    Action, ChannelInfo, Chat, Connect, Connection, ConnectionInfo, Event, Events, HistoryAfter,
    Limits, Message, Outgoing, SendError,
};
use crate::core::config::{ChatConfig, Problem};
use crate::core::{Error, Result as CoreResult, now};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::Write,
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tungstenite::{Message as Frame, WebSocket};
use ureq::unversioned::{
    resolver::{DefaultResolver, Resolver},
    transport::{
        ConnectionDetails, Connector, DefaultConnector, NextTimeout, Transport, TransportAdapter,
        time,
    },
};

pub struct Slack;

#[cfg(test)]
mod tests;

#[cfg(test)]
fn connect_at(options: Connect, url: &str) -> CoreResult<Box<dyn Connection>> {
    let mut api = Api::new(options.proxy.as_deref())?;
    api.base = url.into();
    start(options, api)
}

impl Chat for Slack {
    fn connect(&self, options: Connect) -> crate::core::Result<Box<dyn Connection>> {
        let api = Api::new(options.proxy.as_deref())?;
        start(options, api)
    }
    fn kind(&self) -> &'static str {
        "slack"
    }

    fn secret_keys(&self) -> &'static [&'static str] {
        &["app_token", "bot_token"]
    }

    fn read_config(&self, table: &toml::Table) -> Result<ChatConfig, Problem> {
        let config = ChatConfig::parse(self.kind(), table)?;
        if !id(&config.owner, b"UW") {
            return Err(Problem::new(
                "owner",
                "expected a member ID starting with U or W, followed by uppercase ASCII letters or digits",
            ));
        }
        for (i, channel) in config.channels.iter().enumerate() {
            if !id(&channel.id, b"CG") {
                return Err(Problem::new(
                    format!("channels[{i}].id"),
                    "expected a channel ID starting with C or G, followed by uppercase ASCII letters or digits",
                ));
            }
        }
        Ok(config)
    }
}

struct SlackConnection {
    api: Api,
    session: Arc<Mutex<Session>>,
    worker: Option<JoinHandle<()>>,
    owner: String,
    sent: BTreeMap<String, Instant>,
    problems: BTreeMap<String, String>,
}
impl Connection for SlackConnection {
    fn name(&self) -> &str {
        "Slack"
    }
    fn limits(&self) -> Limits {
        Limits {
            characters: 10_000,
            threads: true,
            buttons: true,
        }
    }
    fn send(&mut self, message: Outgoing<'_>) -> Result<String, SendError> {
        let result = (|| {
            if let Some(sent) = self.sent.get(message.channel) {
                self.api
                    .wait(Duration::from_secs(1).saturating_sub(sent.elapsed()))?;
            }
            let mut body = message_body(&message, &self.owner);
            if let Some(thread) = message.thread {
                body["thread_ts"] = thread.into();
            }
            let result = self.call("chat.postMessage", body);
            // Also space requests after ambiguous network failures; never resend those.
            self.sent.insert(message.channel.into(), Instant::now());
            match result {
                Ok(value) => {
                    self.problems.remove(message.channel);
                    required(&value, "ts")
                }
                Err(e) => {
                    if matches!(e.code.as_str(), "not_member" | "not_found" | "archived") {
                        self.problems.insert(message.channel.into(), e.code.clone());
                    }
                    Err(e)
                }
            }
        })();
        result.map_err(|e: Error| SendError {
            resize: matches!(e.code.as_str(), "message_too_long" | "invalid_blocks"),
            code: e.code,
        })
    }
    fn update(&mut self, id: &str, message: Outgoing<'_>) -> CoreResult<()> {
        let mut body = message_body(&message, &self.owner);
        body["ts"] = id.into();
        self.call("chat.update", body).map(|_| ())
    }
    fn channel(&mut self, id: &str) -> CoreResult<ChannelInfo> {
        let mut info = ChannelInfo {
            id: id.into(),
            name: None,
            private: None,
            problem: None,
        };
        match self.call("conversations.info", json!({"channel":id})) {
            Ok(value) => {
                let channel = value
                    .get("channel")
                    .filter(|v| v.is_object())
                    .ok_or_else(|| error("slack_error"))?;
                info.name = channel["name"].as_str().map(str::to_owned);
                info.private = channel["is_private"].as_bool();
                info.problem = if channel["is_archived"] == true {
                    Some("archived".into())
                } else if channel["is_member"] == false {
                    Some("not_member".into())
                } else if channel["is_member"] == true {
                    self.problems.remove(id);
                    None
                } else {
                    self.problems.get(id).cloned()
                };
            }
            Err(e) if matches!(e.code.as_str(), "not_member" | "not_found" | "archived") => {
                info.problem = Some(e.code)
            }
            Err(e) => return Err(e),
        }
        Ok(info)
    }
    fn history(&mut self, channel: &str, after: &HistoryAfter) -> CoreResult<Vec<Message>> {
        let lower = after
            .id
            .clone()
            .unwrap_or_else(|| format!("{:.6}", after.at));
        let bound = timestamp(&lower)?;
        let parents_since = now() - 30. * 86400.;
        let parents = self.pages(
            "conversations.history",
            json!({"channel":channel,"limit":100,
            "oldest":format!("{:.6}", bound.min(parents_since)),"inclusive":true}),
        )?;
        let (user, bot_id) = {
            let session = self.session.lock().unwrap();
            (session.user.clone(), session.bot_id.clone())
        };
        let mut found = Vec::new();
        for mut parent in parents {
            let ts = required(&parent, "ts")?;
            let at = timestamp(&ts)?;
            parent["channel"] = channel.into();
            if at > bound
                && let Some(Event::Message(m)) = translate(&parent, &user, &bot_id)
            {
                found.push(m);
            }
            if at >= parents_since
                && parent["latest_reply"]
                    .as_str()
                    .is_some_and(|ts| timestamp(ts).is_ok_and(|at| at > bound))
            {
                for mut reply in self.pages(
                    "conversations.replies",
                    json!({"channel":channel,"ts":ts,
                    "oldest":lower,"inclusive":false,"limit":100}),
                )? {
                    reply["channel"] = channel.into();
                    if let Some(Event::Message(m)) = translate(&reply, &user, &bot_id)
                        && m.at > bound
                    {
                        found.push(m);
                    }
                }
            }
        }
        found.sort_by(|a, b| a.at.total_cmp(&b.at));
        found.dedup_by(|a, b| a.id == b.id);
        Ok(found)
    }
    fn exists(&mut self, channel: &str, thread: Option<&str>, id: &str) -> CoreResult<bool> {
        timestamp(id)?;
        let mut body =
            json!({"channel":channel,"oldest":id,"latest":id,"inclusive":true,"limit":100});
        let method = if let Some(thread) = thread {
            body["ts"] = thread.into();
            "conversations.replies"
        } else {
            "conversations.history"
        };
        let messages = self.pages(method, body)?;
        // Only a complete, successful exact-range lookup can establish absence.
        Ok(messages.iter().any(|m| {
            m["ts"] == id && m["subtype"] != "tombstone" && m["subtype"] != "message_deleted"
        }))
    }
    fn close(&mut self) {
        *self.api.stop.0.lock().unwrap() = true;
        self.api.stop.1.notify_all();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn message_body(message: &Outgoing<'_>, owner: &str) -> Value {
    let text = escape(message.markdown);
    let summary: String = text
        .lines()
        .next()
        .unwrap_or("pony 回复")
        .chars()
        .take(200)
        .collect();
    let mut blocks = Vec::new();
    if message.notify {
        blocks.push(json!({"type":"section","text":{"type":"mrkdwn",
            "text":format!("<@{owner}> pony 新消息")}}));
    }
    blocks.push(json!({"type":"markdown","text":text}));
    // Slack allows at most 25 elements in each actions block.
    for chunk in message.buttons.chunks(25) {
        let elements: Vec<_> = chunk
            .iter()
            .map(|button| {
                let mut element = json!({"type":"button", "action_id":button.value,
                "text":{"type":"plain_text","text":button.label}, "value":button.value});
                match button.style {
                    super::ButtonStyle::Default => {}
                    super::ButtonStyle::Primary => element["style"] = "primary".into(),
                    super::ButtonStyle::Danger => element["style"] = "danger".into(),
                }
                if let Some(confirm) = &button.confirm {
                    element["confirm"] = json!({
                    "title":{"type":"plain_text","text":confirm.title},
                    "text":{"type":"plain_text","text":confirm.text},
                    "confirm":{"type":"plain_text","text":confirm.confirm},
                    "deny":{"type":"plain_text","text":confirm.cancel}});
                }
                element
            })
            .collect();
        blocks.push(json!({"type":"actions", "elements":elements}));
    }
    json!({"channel":message.channel,"text":summary,"blocks":blocks,
        "unfurl_links":false,"unfurl_media":false,"parse":"none","link_names":false})
}

impl Drop for SlackConnection {
    fn drop(&mut self) {
        self.close();
    }
}

impl SlackConnection {
    fn call(&self, method: &str, body: Value) -> CoreResult<Value> {
        let token = self.session.lock().unwrap().token.clone();
        if token.is_empty() {
            return Err(error("connecting"));
        }
        self.api.call(method, &token, body, None)
    }
    fn pages(&self, method: &str, mut body: Value) -> CoreResult<Vec<Value>> {
        let mut messages = Vec::new();
        let mut cursors = std::collections::BTreeSet::new();
        loop {
            let value = self.call(method, body.clone())?;
            let page = value["messages"]
                .as_array()
                .ok_or_else(|| error("slack_error"))?;
            if page.iter().any(|m| m["ts"].as_str().is_none()) {
                return Err(error("slack_error"));
            }
            messages.extend(page.iter().cloned());
            let cursor = value["response_metadata"]["next_cursor"]
                .as_str()
                .unwrap_or_default()
                .trim();
            if cursor.is_empty() {
                if value["has_more"] == true {
                    return Err(error("slack_error"));
                }
                return Ok(messages);
            }
            if !cursors.insert(cursor.to_owned()) {
                return Err(error("slack_error"));
            }
            body["cursor"] = cursor.into();
        }
    }
}

fn timestamp(value: &str) -> CoreResult<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|t| t.is_finite() && *t >= 0.)
        .ok_or_else(|| error("slack_error"))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn id(value: &str, prefixes: &[u8]) -> bool {
    let bytes = value.as_bytes();
    bytes.len() > 1
        && prefixes.contains(&bytes[0])
        && bytes[1..]
            .iter()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

// No transport errors, payloads, URLs or headers cross this boundary.
fn error(code: &str) -> Error {
    Error::new(code, format!("Slack: {code}"))
}
fn api_code(raw: &str) -> &'static str {
    match raw {
        "invalid_auth" | "not_authed" | "token_revoked" | "token_expired" | "account_inactive" => {
            "auth_failed"
        }
        "not_in_channel" => "not_member",
        "channel_not_found" => "not_found",
        "is_archived" => "archived",
        "message_too_long" | "msg_too_long" => "message_too_long",
        "invalid_blocks" | "invalid_blocks_format" | "msg_blocks_too_long" => "invalid_blocks",
        "missing_scope" => "missing_scope",
        "thread_not_found" => "thread_not_found",
        "message_not_found" => "message_not_found",
        "cant_update_message" => "cant_update_message",
        "ratelimited" | "rate_limited" => "rate_limited",
        "not_allowed_token_type" => "auth_failed",
        "socket_mode_disabled" => "socket_mode_off",
        _ => "slack_error",
    }
}
#[derive(Clone)]
struct Api {
    agent: ureq::Agent,
    stop: Arc<(Mutex<bool>, Condvar)>,
    heartbeat: Heartbeat,
    #[cfg(test)]
    base: String,
}
#[derive(Clone, Copy)]
struct Heartbeat {
    ping: f64,
    dead: f64,
}

// Only errors raised while building the transport can establish that no HTTP
// request was sent. The same I/O kinds during request/response I/O are ambiguous.
#[derive(Debug, Default)]
struct HttpConnector(DefaultConnector);
impl Connector for HttpConnector {
    type Out = Box<dyn Transport>;
    fn connect(
        &self,
        details: &ConnectionDetails<'_>,
        chained: Option<()>,
    ) -> std::result::Result<Option<Self::Out>, ureq::Error> {
        self.0.connect(details, chained).map_err(connection_error)
    }
}
#[derive(Debug, Default)]
struct HttpResolver(DefaultResolver);
impl Resolver for HttpResolver {
    fn resolve(
        &self,
        uri: &ureq::http::Uri,
        config: &ureq::config::Config,
        timeout: NextTimeout,
    ) -> std::result::Result<ureq::unversioned::resolver::ResolvedSocketAddrs, ureq::Error> {
        self.0.resolve(uri, config, timeout).map_err(|e| match e {
            // getaddrinfo failures arrive as Io too; the resolver has not sent HTTP.
            ureq::Error::Io(_) => ureq::Error::HostNotFound,
            e => e,
        })
    }
}
fn connection_error(e: ureq::Error) -> ureq::Error {
    if let ureq::Error::Io(io) = &e
        && matches!(
            io.kind(),
            std::io::ErrorKind::ConnectionRefused
                | std::io::ErrorKind::NetworkUnreachable
                | std::io::ErrorKind::HostUnreachable
        )
    {
        ureq::Error::ConnectionFailed
    } else {
        e
    }
}
fn unsent(e: &ureq::Error) -> bool {
    matches!(
        e,
        ureq::Error::HostNotFound
            | ureq::Error::ConnectionFailed
            | ureq::Error::ConnectProxyFailed(_)
            | ureq::Error::Timeout(ureq::Timeout::Resolve | ureq::Timeout::Connect)
    )
}

impl Api {
    fn new(proxy: Option<&str>) -> CoreResult<Self> {
        let proxy = proxy
            .map(ureq::Proxy::new)
            .transpose()
            .map_err(|_| error("network"))?;
        // SOCKS is not compiled in. Reject it instead of silently bypassing a proxy.
        if proxy.as_ref().is_some_and(|p| {
            !matches!(
                p.protocol(),
                ureq::ProxyProtocol::Http | ureq::ProxyProtocol::Https
            )
        }) {
            return Err(error("network"));
        }
        let config = ureq::Agent::config_builder()
            .proxy(proxy) // Explicit None disables environment proxies.
            .max_redirects(0) // Never forward credentials to another host.
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(10)))
            .timeout_resolve(Some(Duration::from_secs(3)))
            .timeout_connect(Some(Duration::from_secs(5)))
            .build();
        Ok(Self {
            agent: ureq::Agent::with_parts(
                config,
                HttpConnector::default(),
                HttpResolver::default(),
            ),
            stop: Arc::new((Mutex::new(false), Condvar::new())),
            heartbeat: Heartbeat {
                ping: 30.,
                dead: 90.,
            },
            #[cfg(test)]
            base: "https://slack.com/api/".into(),
        })
    }
    fn stopped(&self) -> bool {
        *self.stop.0.lock().unwrap()
    }
    fn wait(&self, duration: Duration) -> CoreResult<()> {
        let stopped = self.stop.0.lock().unwrap();
        let (stopped, _) = self
            .stop
            .1
            .wait_timeout_while(stopped, duration, |s| !*s)
            .unwrap();
        if *stopped {
            Err(error("stopped"))
        } else {
            Ok(())
        }
    }
    fn call(
        &self,
        method: &str,
        token: &str,
        body: Value,
        on_rate_limit: Option<&dyn Fn() -> CoreResult<()>>,
    ) -> CoreResult<Value> {
        #[cfg(not(test))]
        let base = "https://slack.com/api/";
        #[cfg(test)]
        let base = &self.base;
        let mut network_retries = 0;
        loop {
            if self.stopped() {
                return Err(error("stopped"));
            }
            let url = format!("{base}{method}");
            let response = if matches!(
                method,
                "apps.connections.open" | "chat.postMessage" | "chat.update"
            ) {
                self.agent
                    .post(&url)
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json; charset=utf-8")
                    .send(body.to_string().as_bytes())
            } else {
                let mut request = self
                    .agent
                    .get(&url)
                    .header("Authorization", format!("Bearer {token}"));
                if let Some(params) = body.as_object() {
                    for (key, value) in params {
                        request = request.query(
                            key,
                            value
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| value.to_string()),
                        );
                    }
                }
                request.call()
            };
            let mut response = match response {
                Err(e) if method == "chat.postMessage" && network_retries < 3 && unsent(&e) => {
                    self.wait(Duration::from_secs(1 << network_retries))?;
                    network_retries += 1;
                    continue;
                }
                Err(_) => return Err(error("network")),
                Ok(response) => response,
            };
            if response.status().as_u16() == 429 {
                let seconds = response
                    .headers()
                    .get("retry-after")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(1)
                    .max(1);
                let _ = writeln!(std::io::stderr(), "rate_limited wait_seconds={seconds}");
                if let Some(report) = on_rate_limit {
                    report()?;
                }
                drop(response);
                self.wait(Duration::from_secs(seconds))?;
                continue;
            }
            if response.status().as_u16() == 401 {
                return Err(error("auth_failed"));
            }
            if !response.status().is_success() {
                return Err(error("network"));
            }
            let bytes = response
                .body_mut()
                .with_config()
                .limit(8 * 1024 * 1024)
                .read_to_vec()
                .map_err(|_| error("network"))?;
            let value: Value = serde_json::from_slice(&bytes).map_err(|_| error("slack_error"))?;
            if value["ok"] != true {
                return Err(error(api_code(value["error"].as_str().unwrap_or_default())));
            }
            return Ok(value);
        }
    }
}
#[derive(Default, Clone)]
struct Session {
    token: String,
    user: String,
    bot_id: String,
    workspace: Option<String>,
    bot: Option<String>,
}
fn report(
    events: &Events,
    state: &str,
    reason: Option<&str>,
    session: &Session,
    count: u32,
) -> CoreResult<()> {
    events.emit(Event::Connection(ConnectionInfo {
        connection: state.into(),
        reason: reason.map(str::to_owned),
        message: reason.map(|code| format!("Slack: {code}")),
        workspace: session.workspace.clone(),
        bot: session.bot.clone(),
        other_connections: count,
    }))
}
fn start(options: Connect, api: Api) -> CoreResult<Box<dyn Connection>> {
    let session = Arc::new(Mutex::new(Session::default()));
    let mut connection = SlackConnection {
        api: api.clone(),
        session: session.clone(),
        worker: None,
        owner: options.config.owner.clone(),
        sent: BTreeMap::new(),
        problems: BTreeMap::new(),
    };
    let worker = thread::Builder::new()
        .name("pony-socket".into())
        .spawn(move || {
            let mut delay = 1;
            while !api.stopped() {
                let mut connected_since = None;
                let old = session.lock().unwrap().clone();
                let result = report(&options.events, "connecting", None, &old, 0)
                    .and_then(|()| receive(&api, &options, &session, &mut connected_since));
                if api.stopped() {
                    break;
                }
                let reason = result.err().map_or("network".into(), |e| e.code);
                let old = session.lock().unwrap().clone();
                if report(&options.events, "failed", Some(&reason), &old, 0).is_err() {
                    let _ = writeln!(std::io::stderr(), "event_persist_failed");
                }
                if connected_since.is_some_and(|at| at.elapsed() >= Duration::from_secs(90)) {
                    delay = 1;
                }
                if api.wait(Duration::from_secs(delay)).is_err() {
                    break;
                }
                delay = (delay * 2).min(60);
            }
        })
        .map_err(|_| error("network"))?;
    connection.worker = Some(worker);
    Ok(Box::new(connection))
}
fn timeout(duration: Duration) -> NextTimeout {
    NextTimeout {
        after: time::Duration::Exact(duration),
        reason: ureq::Timeout::Global,
    }
}
fn transport(
    details: &ConnectionDetails<'_>,
) -> std::result::Result<Box<dyn Transport>, ureq::Error> {
    DefaultConnector::default()
        .connect(details, None)?
        .ok_or(ureq::Error::ConnectionFailed)
}
fn open_socket(api: &Api, url: &str) -> CoreResult<WebSocket<TransportAdapter>> {
    let uri: ureq::http::Uri = url.parse().map_err(|_| error("network"))?;
    let host = uri.host().ok_or_else(|| error("network"))?;
    let permitted =
        uri.scheme_str() == Some("wss") && (host.ends_with(".slack.com") || host == "slack.com");
    #[cfg(test)]
    let permitted = permitted || (uri.scheme_str() == Some("ws") && host == "127.0.0.1");
    if !permitted || uri.authority().is_some_and(|a| a.as_str().contains('@')) {
        return Err(error("network"));
    }
    // ureq supplies the rustls-validated stream and HTTP(S) CONNECT proxy support.
    // tungstenite handles only the WebSocket handshake/frames on this stream.
    let uri: ureq::http::Uri = url
        .replacen("wss:", "https:", 1)
        .replacen("ws:", "http:", 1)
        .parse()
        .map_err(|_| error("network"))?;
    let resolver = DefaultResolver::default();
    let config = api.agent.config();
    let wait = timeout(Duration::from_secs(10));
    let addrs = if config.proxy().is_some() {
        resolver.empty()
    } else {
        resolver
            .resolve(&uri, config, wait)
            .map_err(|_| error("network"))?
    };
    let details = ConnectionDetails {
        uri: &uri,
        addrs,
        config,
        request_level: false,
        resolver: &resolver,
        now: time::Instant::now(),
        timeout: wait,
        current_time: Arc::new(time::Instant::now),
        run_connector: Arc::new(transport),
    };
    let mut stream = TransportAdapter::new(transport(&details).map_err(|_| error("network"))?);
    stream.set_timeout(wait);
    let config =
        tungstenite::protocol::WebSocketConfig::default().max_message_size(Some(4 * 1024 * 1024));
    let (mut socket, _) = tungstenite::client::client_with_config(url, stream, Some(config))
        .map_err(|_| error("network"))?;
    socket
        .get_mut()
        .set_timeout(timeout(Duration::from_millis(250)));
    Ok(socket)
}
fn read_timeout(error: &tungstenite::Error) -> bool {
    let tungstenite::Error::Io(e) = error else {
        return false;
    };
    matches!(
        e.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    ) || e
        .get_ref()
        .and_then(|e| e.downcast_ref::<ureq::Error>())
        .is_some_and(|e| matches!(e, ureq::Error::Timeout(_)))
}
fn receive(
    api: &Api,
    options: &Connect,
    shared: &Mutex<Session>,
    connected_since: &mut Option<Instant>,
) -> CoreResult<()> {
    let credentials = (options.credentials)().map_err(|e| {
        error(match e.code.as_str() {
            "secrets_missing" => "secrets_missing",
            "secrets_too_open" => "secrets_too_open",
            _ => "secrets_invalid",
        })
    })?;
    let token = |key| {
        credentials
            .0
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| error("secrets_invalid"))
    };
    let limited = || {
        report(
            &options.events,
            "failed",
            Some("rate_limited"),
            &shared.lock().unwrap(),
            0,
        )
    };
    let opened = api.call(
        "apps.connections.open",
        token("app_token")?,
        json!({}),
        Some(&limited),
    )?;
    let auth = api.call("auth.test", token("bot_token")?, json!({}), Some(&limited))?;
    let session = Session {
        token: token("bot_token")?.into(),
        user: required(&auth, "user_id")?,
        bot_id: required(&auth, "bot_id")?,
        workspace: Some(required(&auth, "team")?),
        bot: Some(required(&auth, "user")?),
    };
    let mut socket = open_socket(api, &required(&opened, "url")?)?;
    let mut last_received = now();
    let mut last_ping = last_received;
    let mut connected = false;
    while !api.stopped() {
        let at = now();
        if at - last_received >= api.heartbeat.dead {
            return Err(error("network"));
        }
        if at - last_ping >= api.heartbeat.ping {
            socket
                .send(Frame::Ping(Vec::new().into()))
                .map_err(|_| error("network"))?;
            last_ping = at;
        }
        let frame = match socket.read() {
            Ok(frame) => frame,
            Err(e) if read_timeout(&e) => continue,
            Err(_) => return Err(error("network")),
        };
        last_received = now();
        let Frame::Text(text) = frame else {
            if matches!(frame, Frame::Close(_)) {
                return Err(error("network"));
            }
            socket.flush().map_err(|_| error("network"))?; // Includes automatic pong.
            continue;
        };
        let value: Value = serde_json::from_str(&text).map_err(|_| error("slack_error"))?;
        match value["type"].as_str() {
            Some("hello") if !connected => {
                *shared.lock().unwrap() = session.clone();
                let count = value["num_connections"]
                    .as_u64()
                    .unwrap_or(1)
                    .saturating_sub(1)
                    .min(u32::MAX as u64) as u32;
                report(&options.events, "connected", None, &session, count)?;
                *connected_since = Some(Instant::now());
                connected = true;
            }
            Some("disconnect") => {
                return Err(error(if value["reason"] == "link_disabled" {
                    "socket_mode_off"
                } else {
                    "network"
                }));
            }
            _ => {}
        }
        if let Some(envelope) = value["envelope_id"].as_str() {
            if !connected {
                return Err(error("network"));
            }
            emit_received(&value, options, &session)?;
            socket
                .send(Frame::Text(
                    json!({"envelope_id":envelope}).to_string().into(),
                ))
                .map_err(|_| error("network"))?;
        }
    }
    Ok(())
}
fn emit_received(value: &Value, options: &Connect, session: &Session) -> CoreResult<()> {
    if value["type"] == "interactive" {
        let payload = &value["payload"];
        if payload["type"] != "block_actions" || payload["container"]["type"] != "message" {
            return Ok(());
        }
        let channel = required(&payload["channel"], "id")?;
        let message = required(&payload["container"], "message_ts")?;
        let sender = required(&payload["user"], "id")?;
        let own_message = payload["message"]["user"] == session.user
            && payload["message"]["bot_id"] == session.bot_id
            && payload["message"]["ts"] == message
            && payload["container"]["channel_id"] == channel;
        if let Some(actions) = payload["actions"].as_array() {
            for action in actions.iter().filter(|a| a["type"] == "button") {
                options.events.emit(Event::Action(Action {
                    channel: channel.clone(),
                    message: message.clone(),
                    thread: payload["message"]["thread_ts"].as_str().map(str::to_owned),
                    sender: sender.clone(),
                    value: required(action, "value")?,
                    at: timestamp(&required(action, "action_ts")?)?,
                    own_message,
                }))?;
            }
        }
        return Ok(());
    }
    if value["type"] != "events_api" {
        return Ok(());
    }
    let Some(event) = translate(&value["payload"]["event"], &session.user, &session.bot_id) else {
        // Unsupported events need no durable record; the caller still ACKs them.
        return Ok(());
    };
    if let Event::Message(message) = &event
        && message.sender != options.config.owner
        && !options
            .config
            .channels
            .iter()
            .any(|channel| channel.id == message.channel)
    {
        return Ok(());
    }
    options.events.emit(event)
}
fn required(value: &Value, field: &str) -> CoreResult<String> {
    value[field]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| error("slack_error"))
}
fn translate(value: &Value, bot: &str, bot_id: &str) -> Option<Event> {
    if value["type"].as_str().is_some_and(|t| t != "message") || value.get("bot_id").is_some() {
        return None;
    }
    let channel = value["channel"].as_str()?.to_owned();
    match value["subtype"].as_str() {
        Some("message_deleted") => {
            return Some(Event::Deleted {
                channel,
                id: value["deleted_ts"].as_str()?.into(),
            });
        }
        None | Some("file_share") | Some("thread_broadcast") => {}
        _ => return None,
    }
    let id = value["ts"].as_str()?.to_owned();
    let at: f64 = id.parse().ok()?;
    if !at.is_finite() {
        return None;
    }
    let sender = value["user"].as_str()?.to_owned();
    if sender == bot {
        return None;
    }
    let text = value["text"].as_str().unwrap_or_default();
    let stripped = [bot, bot_id]
        .iter()
        .find_map(|id| text.trim_start().strip_prefix(&format!("<@{id}>")));
    let mentioned = stripped.is_some();
    let text = stripped.map(str::trim_start).unwrap_or(text);
    Some(Event::Message(Message {
        channel,
        thread: value["thread_ts"]
            .as_str()
            .filter(|t| *t != id)
            .map(str::to_owned),
        id,
        sender,
        at,
        mentioned,
        text: decode(text),
        attachments: value["subtype"] == "file_share"
            || value["files"].as_array().is_some_and(|a| !a.is_empty())
            || value["attachments"].as_array().is_some_and(|a| {
                a.iter().any(|item| {
                    item["is_share"] == true
                        || (item.get("from_url").is_none() && item.get("original_url").is_none())
                })
            }),
    }))
}
fn decode(text: &str) -> String {
    let mut output = String::new();
    let mut rest = text;
    while let Some((before, tail)) = rest.split_once('<') {
        output.push_str(before);
        if let Some((inside, after)) = tail.split_once('>') {
            if inside.starts_with("https://")
                || inside.starts_with("http://")
                || inside.starts_with("mailto:")
            {
                output.push_str(inside.split('|').next().unwrap());
            } else {
                output.push('<');
                output.push_str(inside);
                output.push('>');
            }
            rest = after;
        } else {
            output.push('<');
            rest = tail;
            break;
        }
    }
    output.push_str(rest);
    output
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}
