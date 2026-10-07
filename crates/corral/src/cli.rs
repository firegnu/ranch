// From Saddle `crates/corral-core/src/cli.rs` at commit `a31dea2`; ranch has changed it since.
use crate::{Error, Result, events, hooks, now, pen, state};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};
struct Args {
    words: Vec<String>,
    options: BTreeMap<String, Vec<String>>,
}
impl Args {
    fn parse(args: &[String], flags: &[&str], values: &[&str]) -> Result<Self> {
        let mut words = Vec::new();
        let mut options: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut i = 0;
        let mut literal = false;
        while i < args.len() {
            let a = &args[i];
            if a == "--" && !literal {
                literal = true;
                i += 1;
                continue;
            }
            if !literal && a.starts_with('-') {
                let (key, inline) = a
                    .split_once('=')
                    .map_or((a.as_str(), None), |(k, v)| (k, Some(v.to_owned())));
                let value = if flags.contains(&key) && inline.is_none() {
                    String::new()
                } else if values.contains(&key) {
                    if let Some(v) = inline {
                        v
                    } else {
                        i += 1;
                        args.get(i)
                            .ok_or_else(|| usage("missing option value"))?
                            .clone()
                    }
                } else {
                    return Err(usage(format!("unrecognized argument {a}")));
                };
                options.entry(key.into()).or_default().push(value);
            } else {
                words.push(a.clone());
            }
            i += 1;
        }
        Ok(Self { words, options })
    }
    fn has(&self, k: &str) -> bool {
        self.options.contains_key(k)
    }
    fn get(&self, k: &str) -> Option<&str> {
        self.options
            .get(k)
            .and_then(|v| v.last())
            .map(String::as_str)
    }
    fn seconds(&self, k: &str, default: f64) -> Result<f64> {
        self.get(k)
            .map(|v| {
                v.parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| usage("invalid timeout"))
            })
            .unwrap_or(Ok(default))
    }
    fn count(&self, n: usize) -> Result<()> {
        if self.words.len() != n {
            Err(usage("unexpected or missing positional argument"))
        } else {
            Ok(())
        }
    }
    fn pairs(&self, k: &str, labels: bool) -> Result<Value> {
        let mut obj = json!({});
        for s in self.options.get(k).into_iter().flatten() {
            let (key, value) = s
                .split_once('=')
                .filter(|(k, _)| !k.is_empty())
                .ok_or_else(|| usage("expected KEY=VALUE"))?;
            if labels
                && (!key.as_bytes()[0].is_ascii_alphanumeric()
                    || !key
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)))
            {
                return Err(usage("invalid label key"));
            }
            obj[key] = json!(value);
        }
        Ok(obj)
    }
}
fn usage(message: impl ToString) -> Error {
    Error::new(1, "usage", message)
}
pub(crate) struct Status {
    pub(crate) public: Value,
    pub(crate) snapshot: Option<Value>,
    pub(crate) pen: Value,
}
pub(crate) fn status(name: &str) -> Result<Status> {
    let st = state::require(name, json!({"op":"status"}))?;
    let d = state::dir(name)?;
    let m = state::read(d.join("meta.json"));
    let l = state::read(d.join("labels.json"));
    let labels = if l["instance"] == st["instance"] && l["labels"].is_object() {
        l["labels"].clone()
    } else {
        json!({})
    };
    let mut public = json!({"ok":true,"name":name,"instance":st["instance"],"kind":m["kind"],"proto":st["proto"],"state":"unknown","state_started":null,"last_tool":null,"turn_started":null,"last_event":null,"last_event_at":null,"last_input_at":null,"last_input_source":null,"title":st["title"],"last_output":st["last_output"],"idle_for":st["last_output"].as_f64().map(|t|((now()-t)*1000.0).round()/1000.0),"attached":st["attached"],"last_human_input":st["last_human_input"],"started":st["started"],"labels":labels,"paused":st["paused"] == true,"paused_at":st["paused_at"]});
    for key in [
        "exe",
        "custody",
        "capabilities",
        "upgrade",
        "agent_pid",
        "pen_pid",
        "recent_sends",
    ] {
        if let Some(value) = st.get(key) {
            public[key] = value.clone();
        }
    }
    let snapshot = if hooks::known(m["kind"].as_str().unwrap_or("")) {
        let snap = events::read(
            &d,
            st["instance"].as_str().unwrap_or(""),
            Path::new(m["cwd"].as_str().unwrap_or("")),
        )?;
        public["state"] =
            if m["has_prompt"] == true && snap["input_count"] == 0 && snap["state"] == "idle" {
                json!("starting")
            } else {
                snap["state"].clone()
            };
        if public["state"] == snap["state"] {
            public["state_started"] = snap["state_started"].clone();
        }
        for (a, b) in [
            ("last_tool", "last_tool"),
            ("turn_started", "turn_started"),
            ("last_event", "last_event"),
            ("last_event_at", "last_event_t"),
        ] {
            public[a] = snap[b].clone();
        }
        if let Some(input) = snap["inputs"].as_array().and_then(|a| a.last()) {
            public["last_input_at"] = input["t"].clone();
            let t = input["t"].as_f64().unwrap_or(0.0);
            public["last_input_source"] =
                json!(if st["recent_sends"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|r| r["digest"] == input["digest"]
                        && r["t"].as_f64().unwrap_or(f64::MAX) <= t))
                {
                    "send"
                } else if st["last_human_input"]
                    .as_f64()
                    .is_some_and(|h| h >= t - 2.0 && h <= t + 0.5)
                {
                    "human"
                } else {
                    "agent"
                });
        }
        Some(snap)
    } else {
        None
    };
    Ok(Status {
        public,
        snapshot,
        pen: st,
    })
}
fn deliver(
    name: &str,
    text: &str,
    force: bool,
    timeout: f64,
    st: Status,
    request_id: &str,
) -> Result<Value> {
    let t0 = now();
    send_once(name, text, force, &st, request_id)?;
    if st.snapshot.is_none() {
        return Ok(
            json!({"ok":true,"name":name,"instance":st.public["instance"],"request_id":request_id,"confirmed":false}),
        );
    }
    let deadline = now() + timeout;
    loop {
        if let Some(mut result) =
            confirmation(name, text, st.public["instance"].as_str().unwrap_or(""), t0)?
        {
            result["request_id"] = json!(request_id);
            return Ok(result);
        }
        if now() >= deadline {
            // Accepted, but a paused agent cannot read it: that is not a failed delivery.
            let pen = state::require(name, json!({"op":"status"}))?;
            if pen["instance"] == st.public["instance"]
                && (pen["paused"] == true || pen["resumed_at"].as_f64().is_some_and(|t| t >= t0))
            {
                return Ok(
                    json!({"ok":true,"name":name,"instance":pen["instance"],"request_id":request_id,"confirmed":false,"paused":true,"message":"accepted; the agent was paused before it read the input; not resending"}),
                );
            }
            return Err(
                Error::new(3, "not_delivered", "no matching input event; not resending")
                    .with("name", name)
                    .with("instance", st.public["instance"].clone())
                    .with("request_id", request_id),
            );
        }
        thread::sleep(Duration::from_millis(100));
    }
}
pub(crate) fn send_once(
    name: &str,
    text: &str,
    force: bool,
    st: &Status,
    request_id: &str,
) -> Result<Value> {
    if st.public["paused"] == true {
        return Err(Error::new(
            10,
            "paused",
            format!("{name} is paused; corral resume {name} first"),
        )
        .with("name", name));
    }
    let known = st.snapshot.is_some();
    if known && st.public["state"] != "idle" {
        return Err(Error::new(7, "not_idle", format!("{name} is not idle"))
            .with("name", name)
            .with("state", st.public["state"].clone()));
    }
    let body = if known && st.pen["bracketed_paste"] == true {
        [b"\x1b[200~".as_slice(), text.as_bytes(), b"\x1b[201~"].concat()
    } else {
        text.as_bytes().to_vec()
    };
    let reply = state::request(
        name,
        json!({"op":"send","instance":st.public["instance"],"request_id":request_id,"digest":events::digest(text),"force":force,"chunks":[{"data":B64.encode(body),"delay":0.3},{"data":B64.encode(b"\r")}]}),
    )?;
    if reply["ok"] != true {
        return Err(Error::new(
            if reply["error"] == "human_active" {
                8
            } else if reply["error"] == "paused" {
                10
            } else {
                1
            },
            reply["error"].as_str().unwrap_or("pen_error"),
            reply["message"].as_str().unwrap_or("pen error"),
        )
        .with("name", name)
        .with("last_human_input", reply["last_human_input"].clone()));
    }
    Ok(reply)
}
pub(crate) fn confirmation(
    name: &str,
    text: &str,
    instance: &str,
    t0: f64,
) -> Result<Option<Value>> {
    let d = state::dir(name)?;
    let m = state::read(d.join("meta.json"));
    let snap = events::read(&d, instance, Path::new(m["cwd"].as_str().unwrap_or("")))?;
    let inputs = snap["inputs"].as_array().unwrap();
    let want = events::digest(text);
    let merged = if inputs
        .iter()
        .any(|i| i["digest"] == want && i["t"].as_f64().unwrap_or(0.0) >= t0)
    {
        Some(false)
    } else if inputs
        .last()
        .is_some_and(|i| i["t"].as_f64().unwrap_or(0.0) >= t0)
        && !events::normalize(text).is_empty()
        && snap["last_prompt"]
            .as_str()
            .is_some_and(|p| events::normalize(p).contains(&events::normalize(text)))
    {
        Some(true)
    } else {
        None
    };
    Ok(merged.map(|merged|json!({"ok":true,"name":name,"instance":instance,"confirmed":true,"merged_with_draft":merged,"latency":((now()-t0)*1000.0).round()/1000.0})))
}
fn turn_end(name: &str, timeout: f64, quiet: Option<f64>, instance: Option<&str>) -> Result<Value> {
    let deadline = now() + timeout;
    let mut stable: Option<(Value, f64)> = None;
    loop {
        let full = status(name)?;
        let st = full.public;
        let paused = st["paused"] == true;
        let state = st["state"].as_str().unwrap_or("unknown");
        let key = json!([st["state"], st["last_event"], st["last_event_at"]]);
        let result = if instance.is_some_and(|i| st["instance"] != i) {
            Some("restarted")
        } else if paused {
            // A paused agent has neither finished nor gone quiet, whatever its kind.
            stable = None;
            None
        } else if state == "unknown" {
            Some("unknown")
        } else if matches!(state, "idle" | "blocked") {
            if stable
                .as_ref()
                .is_some_and(|(k, t)| k == &key && now() - t >= 0.5)
            {
                Some(state)
            } else {
                if stable.as_ref().is_none_or(|(k, _)| k != &key) {
                    stable = Some((key, now()));
                }
                None
            }
        } else {
            stable = None;
            if state == "working"
                && quiet.is_some_and(|q| {
                    now()
                        - st["last_output"]
                            .as_f64()
                            .unwrap_or(0.0)
                            .max(st["last_event_at"].as_f64().unwrap_or(0.0))
                            .max(full.pen["resumed_at"].as_f64().unwrap_or(0.0))
                        >= q
                })
            {
                Some("stopped-quiet")
            } else {
                None
            }
        };
        if let Some(result) = result {
            let result = result.to_owned();
            let mut st = st;
            st["result"] = json!(result);
            return Ok(st);
        }
        if now() >= deadline {
            let state = if paused { "paused" } else { state };
            let mut e = Error::new(4, "timeout", format!("{name} still {state}"));
            for (k, v) in st.as_object().unwrap() {
                if k != "ok" {
                    e.value[k] = v.clone();
                }
            }
            return Err(e);
        }
        thread::sleep(Duration::from_millis(200));
    }
}
pub fn run(args: &[String]) -> Result<Option<Value>> {
    if args == ["--version"] {
        return Ok(Some(
            json!({"ok":true,"version":env!("CARGO_PKG_VERSION"),"contract":"1"}),
        ));
    }
    let op = args.first().map(String::as_str).unwrap_or("");
    if matches!(op, "--help" | "-h") {
        println!(
            "corral start|send|keys|status|wait|reply|where|ls|read|attach|stop|pause|resume|upgrade|recover|after|guide|install-skills"
        );
        return Ok(None);
    }
    let help = match op {
        "start" => Some(
            "NAME [--cwd DIR] [--unique] [--prompt TEXT] [--env KEY=VALUE] [--label KEY=VALUE] -- COMMAND [ARG ...]",
        ),
        "send" => Some("NAME TEXT [--force] [--timeout SECONDS] [--after NAME] [--request-id ID]"),
        "keys" => Some("NAME KEY [KEY ...]"),
        "status" | "reply" | "where" | "pause" | "resume" => Some("NAME"),
        "wait" => Some("NAME [--timeout SECONDS] [--quiet SECONDS]"),
        "read" => Some("NAME [--bytes COUNT]"),
        "attach" => Some("NAME [--wait]"),
        "stop" => Some("NAME [--timeout SECONDS]"),
        "after" => Some("NAME [--request-id ID]"),
        "upgrade" | "recover" => Some("[--all|NAME] [--exe PATH]"),
        "ls" | "guide" => Some(""),
        "install-skills" => {
            Some("[--target all|claude|codex] [--project DIR] [--remove] [--dry-run] [--yes]")
        }
        _ => None,
    };
    if let Some(usage) = help
        && args
            .iter()
            .skip(1)
            .take_while(|a| *a != "--")
            .any(|a| matches!(a.as_str(), "--help" | "-h"))
    {
        println!(
            "Usage: corral {op} {usage}\n\nUse corral guide for the complete command contract."
        );
        return Ok(None);
    }
    if op != "guide" && std::env::var_os("CODEX_SANDBOX").is_some() {
        return Err(Error::new(6, "sandbox", "running inside a Codex sandbox"));
    }
    if op == "guide" {
        if args.len() != 1 {
            return Err(usage("unexpected guide arguments"));
        }
        print!("{}", include_str!("../resources/AGENT_USAGE.md"));
        return Ok(None);
    }
    if op == "install-skills" {
        let p = Args::parse(
            &args[1..],
            &["--remove", "--dry-run", "--yes"],
            &["--target", "--project"],
        )?;
        p.count(0)?;
        return Ok(Some(crate::skills::run(
            p.get("--target").unwrap_or("all"),
            p.has("--remove"),
            p.has("--dry-run"),
            p.has("--yes"),
            p.get("--project"),
        )?));
    }
    if op == "after" {
        let p = Args::parse(&args[1..], &[], &["--request-id"])?;
        p.count(1)?;
        return Ok(Some(crate::after::list(
            &p.words[0],
            p.get("--request-id"),
        )?));
    }
    if matches!(op, "upgrade" | "recover") {
        let p = Args::parse(&args[1..], &["--all"], &["--exe"])?;
        p.count(if p.has("--all") { 0 } else { 1 })?;
        let target = p
            .get("--exe")
            .map(PathBuf::from)
            .unwrap_or(crate::executable()?);
        if !target.is_absolute() {
            return Err(usage("--exe must be absolute"));
        }
        if let Some(name) = p.words.first() {
            state::validate(name)?;
        }
        return Ok(Some(crate::upgrade::run(
            p.words.first().map(String::as_str),
            &target,
            op == "recover",
        )?));
    }
    let (flags, values): (&[&str], &[&str]) = match op {
        "start" => (&["--unique"], &["--cwd", "--prompt", "--env", "--label"]),
        "send" => (&["--force"], &["--timeout", "--after", "--request-id"]),
        "wait" => (&[], &["--timeout", "--quiet"]),
        "stop" => (&[], &["--timeout"]),
        "read" => (&[], &["--bytes"]),
        "attach" => (&["--wait"], &[]),
        "status" | "reply" | "where" | "ls" | "keys" | "pause" | "resume" => (&[], &[]),
        _ => return Err(usage("unknown or missing command")),
    };
    let sep = if op == "start" {
        args.iter()
            .position(|a| a == "--")
            .ok_or_else(|| usage("start needs an agent command after --"))?
    } else {
        args.len()
    };
    let parsed = Args::parse(&args[1..sep], flags, values)?;
    if op == "ls" {
        parsed.count(0)?;
        return Ok(Some(json!({"ok":true,"agents":state::list()?})));
    }
    if op == "keys" {
        if parsed.words.len() < 2 {
            return Err(usage("missing keys"));
        }
    } else {
        parsed.count(if op == "send" { 2 } else { 1 })?;
    }
    let name = &parsed.words[0];
    state::validate(name)?;
    let value = match op {
        "start" => {
            if args.len() <= sep + 1 {
                return Err(usage("missing agent command"));
            }
            let cwd = std::path::absolute(
                parsed
                    .get("--cwd")
                    .map(PathBuf::from)
                    .unwrap_or(std::env::current_dir()?),
            )?;
            if !cwd.is_dir() {
                return Err(Error::new(1, "bad_cwd", "no such directory").with("cwd", json!(cwd)));
            }
            let argv = &args[sep + 1..];
            let kind = Path::new(&argv[0])
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            if parsed.has("--prompt") && !hooks::known(&kind) {
                return Err(usage("--prompt requires a recognized agent"));
            }
            pen::start(
                json!({"name":name,"unique":parsed.has("--unique"),"cwd":cwd,"argv":argv,"prompt":parsed.get("--prompt"),"env":parsed.pairs("--env",false)?,"labels":parsed.pairs("--label",true)?}),
            )?
        }
        "status" => status(name)?.public,
        "pause" | "resume" => {
            let reply = state::request(name, json!({"op":op}))?;
            if reply["error"] == "bad_op" {
                return Err(Error::new(
                    9,
                    "unsupported",
                    format!(
                        "{name} runs under a corral without pause; corral upgrade {name} first"
                    ),
                )
                .with("name", name.as_str()));
            }
            if reply["ok"] != true {
                return Err(Error::new(
                    1,
                    reply["error"].as_str().unwrap_or("pen_error"),
                    reply["message"].as_str().unwrap_or("pen error"),
                )
                .with("name", name.as_str()));
            }
            json!({"ok":true,"name":name,"instance":reply["instance"],"paused":reply["paused"],"paused_at":reply["paused_at"]})
        }
        "send" => {
            if let Some(after) = parsed.get("--after") {
                if !parsed.has("--timeout") {
                    return Err(usage("--after needs an explicit --timeout"));
                }
                crate::after::create(
                    name,
                    after,
                    &parsed.words[1],
                    parsed.has("--force"),
                    parsed.seconds("--timeout", 0.0)?,
                    parsed.get("--request-id"),
                )?
            } else {
                deliver(
                    name,
                    &parsed.words[1],
                    parsed.has("--force"),
                    parsed.seconds("--timeout", 15.0)?,
                    status(name)?,
                    &parsed
                        .get("--request-id")
                        .map(str::to_owned)
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                )?
            }
        }
        "wait" => turn_end(
            name,
            parsed.seconds("--timeout", 600.0)?,
            if parsed.has("--quiet") {
                Some(parsed.seconds("--quiet", 0.0)?)
            } else {
                None
            },
            None,
        )?,
        "reply" => {
            let s = status(name)?;
            let snap = s
                .snapshot
                .filter(|s| !s["reply"].is_null())
                .ok_or_else(|| {
                    Error::new(1, "no_reply", "no finished turn with a reply yet")
                        .with("name", name.as_str())
                })?;
            json!({"ok":true,"name":name,"instance":s.public["instance"],"text":snap["reply"],"at":snap["reply_t"]})
        }
        "where" => {
            let s = state::require(name, json!({"op":"status"}))?;
            let m = state::read(state::dir(name)?.join("meta.json"));
            json!({"ok":true,"name":name,"instance":s["instance"],"kind":m["kind"],"cwd":m["cwd"],"agent_pid":s["agent_pid"],"started":s["started"]})
        }
        "read" => {
            let n = parsed
                .get("--bytes")
                .unwrap_or("16000")
                .parse::<i64>()
                .map_err(|_| usage("invalid byte count"))?
                .max(0);
            let r = state::require(name, json!({"op":"read","bytes":n}))?;
            let bytes = B64
                .decode(r["data"].as_str().unwrap_or(""))
                .map_err(|e| Error::new(1, "bad_reply", e))?;
            print!("{}", crate::terminal::strip(&bytes));
            return Ok(None);
        }
        "attach" => {
            crate::attach::run(name, parsed.has("--wait"))?;
            return Ok(None);
        }
        "keys" => {
            let mut chunks = Vec::new();
            for key in &parsed.words[1..] {
                let bytes = if let Some(s) = key.strip_prefix("text:") {
                    s.as_bytes().to_vec()
                } else {
                    match key.as_str() {
                        "enter" => b"\r".to_vec(),
                        "esc" => b"\x1b".to_vec(),
                        "tab" => b"\t".to_vec(),
                        "backspace" => b"\x7f".to_vec(),
                        "space" => b" ".to_vec(),
                        "up" => b"\x1b[A".to_vec(),
                        "down" => b"\x1b[B".to_vec(),
                        "right" => b"\x1b[C".to_vec(),
                        "left" => b"\x1b[D".to_vec(),
                        "ctrl-c" => b"\x03".to_vec(),
                        "ctrl-d" => b"\x04".to_vec(),
                        _ => return Err(usage("unknown key")),
                    }
                };
                chunks.push(json!({"data":B64.encode(bytes),"delay":0.05}));
            }
            state::require(name, json!({"op":"keys","chunks":chunks}))?;
            json!({"ok":true,"name":name})
        }
        "stop" => {
            let timeout = parsed.seconds("--timeout", 90.0)?;
            let s = state::require(name, json!({"op":"status"}))?;
            let m = state::read(state::dir(name)?.join("meta.json"));
            state::require(
                name,
                json!({"op":"stop","steps":hooks::quit_steps(m["kind"].as_str().unwrap_or(""))}),
            )?;
            let deadline = now() + timeout.max(0.0);
            loop {
                let d = state::dir(name)?;
                if std::os::unix::net::UnixStream::connect(state::socket(name)?).is_err()
                    && (!d.join("lock").exists() || state::lock(&d.join("lock"), false)?.is_some())
                {
                    break;
                }
                if now() >= deadline {
                    return Err(
                        Error::new(4, "timeout", "pen keeps stopping").with("name", name.as_str())
                    );
                }
                thread::sleep(Duration::from_millis(50));
            }
            let e = state::read(state::dir(name)?.join("exit.json"));
            let matched = e["instance"] == s["instance"];
            json!({"ok":true,"name":name,"instance":s["instance"],"exit_code":if matched{e["code"].clone()}else{Value::Null},"stopped_by":if matched{e["stop_step"].clone()}else{Value::Null}})
        }
        _ => unreachable!(),
    };
    Ok(Some(value))
}
