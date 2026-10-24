//! Kimi Code 2.3: shared config hooks and the main agent's wire (R48c).
use super::{Adapter, Global, Launch};
use crate::Result;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub struct Kimi;
impl Adapter for Kimi {
    fn kind(&self) -> &'static str {
        "kimi"
    }
    fn hook_args(&self, _launch: &Launch) -> Result<Vec<String>> {
        Ok(vec![])
    }
    fn quit_steps(&self) -> Value {
        json!([{"signal":"TERM","wait":3}])
    }
    fn stop_descendants(&self) -> bool {
        true
    }
    fn skill_targets(&self) -> &'static [&'static str] {
        &["codex"]
    }
    fn prompt_via_terminal(&self) -> bool {
        true
    }
    fn hook_output(&self) -> &'static str {
        ""
    }
    fn hook(&self, event: &str, input: &Value) -> Vec<Value> {
        let Some(session) = input["session_id"].as_str().filter(|s| {
            !s.is_empty()
                && s.len() < 200
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        }) else {
            return vec![];
        };
        let cwd = std::env::var_os("CORRAL_CWD").and_then(|p| std::fs::canonicalize(p).ok());
        if cwd.is_none()
            || input["cwd"]
                .as_str()
                .and_then(|p| std::fs::canonicalize(p).ok())
                != cwd
        {
            return vec![];
        }
        // Child turns share the session and process; their origin is the only hook discriminator.
        if event == "TurnStarted" && input["origin_kind"] != "user" {
            return vec![];
        }
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        let wire = config(&home)
            .join("sessions")
            .join(workdir_key(&cwd.unwrap()))
            .join(session)
            .join("agents/main/wire.jsonl");
        let mut e = match event {
            "SessionStart" => json!({"ev":"SessionStart","source":input["source"]}),
            "UserPromptSubmit" => {
                json!({"ev":"UserPromptSubmit","prompt":input["prompt"].as_array()
                .into_iter().flatten().filter(|p| p["type"] == "text").filter_map(|p| p["text"].as_str()).collect::<String>()})
            }
            // Tools, permissions and Stop lack main/child identity. Keep the observation, but
            // let the main wire decide all state transitions (also after a Stop hook returns).
            _ => json!({"ev":format!("Kimi:{event}")}),
        };
        e["session_id"] = json!(session);
        e["has_transcript"] = json!(true);
        e["transcript_path"] = json!(wire);
        vec![e]
    }
    fn transcript_events(&self, snapshot: &mut Value) -> Vec<Value> {
        wire_events(snapshot)
    }
    fn input_ready(&self, screen: &str) -> bool {
        let bottom: Vec<_> = screen
            .lines()
            .filter(|l| !l.trim().is_empty())
            .rev()
            .take(16)
            .collect();
        bottom
            .iter()
            .any(|l| l.trim().trim_matches('│').trim() == ">")
            && bottom
                .iter()
                .any(|l| l.trim().starts_with("context:") && l.contains('%'))
            && !["/login", "Trust this folder?", "Select a model", "Loading"]
                .iter()
                .any(|s| screen.contains(s))
    }
    fn screen_rules(&self) -> &'static [crate::screen::Rule] {
        &[
            crate::screen::Rule {
                rows: 8,
                all: &["Approve once", "Reject", "choose", "↵ confirm"],
                none: &[],
            },
            crate::screen::Rule {
                rows: 4,
                all: &["select", "choose", "←/→/tab switch", "esc cancel"],
                none: &[],
            },
            crate::screen::Rule {
                rows: 32,
                all: &["Select messages to undo", "Esc cancel"],
                none: &[],
            },
        ]
    }
    fn global(&self) -> Option<Global> {
        Some(Global {
            dir: config,
            path: "config.toml",
            files: &[("", include_str!("../../resources/kimi_hooks.toml"))],
            enabled: None,
            helper: true,
            block: Some((
                "# >>> corral kimi integration",
                "# <<< corral kimi integration",
            )),
        })
    }
}

fn config(home: &Path) -> PathBuf {
    std::env::var_os("KIMI_CODE_HOME")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".kimi-code"))
}

/// Kimi 2.3's encodeWorkDirKey, applied to the actual canonical working directory.
fn workdir_key(cwd: &Path) -> String {
    let path = cwd.to_string_lossy().replace('\\', "/");
    let path = path.trim_end_matches('/');
    let base = path.rsplit('/').next().unwrap_or("").to_lowercase();
    let mut slug = String::new();
    let mut invalid = false;
    for c in base.chars() {
        if c.is_ascii_alphanumeric() || "._-".contains(c) {
            slug.push(c);
            invalid = false;
        } else if !invalid {
            slug.push('-');
            invalid = true;
        }
    }
    let slug: String = slug.trim_matches('-').chars().take(40).collect();
    let slug = slug.trim_matches('-');
    let slug = if ["", ".", ".."].contains(&slug) {
        "workspace"
    } else {
        slug
    };
    format!(
        "wd_{slug}_{}",
        &format!("{:x}", Sha256::digest(path.as_bytes()))[..12]
    )
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct WireCursor {
    version: u8,
    path: String,
    dev: u64,
    ino: u64,
    len: u64,
    offset: u64,
    turn_started: f64,
    turn: Value,
    step: Value,
    text: String,
    tool: Value,
    interaction: Value,
    state: Option<Value>,
    interactions: Vec<Value>,
    background_calls: Vec<String>,
    running: bool,
    prompt_at: f64,
    resolved: Value,
}

// Same compact JSON / Unicode-prefix budget as R43, without changing shared hooks.
fn text_prefix(input: &str, remaining: &mut usize, truncated: &mut bool) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        let size = serde_json::to_string(&ch).unwrap().len() - 2;
        let Some(left) = remaining.checked_sub(size) else {
            *truncated = true;
            break;
        };
        *remaining = left;
        out.push(ch);
    }
    out
}

fn bounded_text(input: &Value) -> Value {
    let Some(input) = input.as_str() else {
        return Value::Null;
    };
    let mut truncated = false;
    let mut remaining = 16 * 1024 - json!({"text":"","truncated":false}).to_string().len();
    let text = text_prefix(input, &mut remaining, &mut truncated);
    json!({"text":text,"truncated":truncated})
}

fn resolved(dialog: &Value, event: &Value, at: f64) -> Value {
    let mut out = json!({"kind":dialog["kind"],"state_started":dialog["at"],"at":at});
    if dialog["kind"] == "approval" {
        out["decision"] = match event["response"]["decision"].as_str() {
            Some("approved") => json!("approved"),
            Some("rejected") => json!("rejected"),
            _ => Value::Null,
        };
        return out;
    }
    out["answers"] = Value::Null;
    out["truncated"] = json!(false);
    if let Some(answers) = event["response"]["answers"].as_object() {
        out["answers"] = json!({});
        let mut remaining = 16 * 1024 - out.to_string().len();
        let mut truncated = false;
        for (question, answer) in answers {
            let Some(answer) = answer.as_str() else {
                truncated = true;
                continue;
            };
            let overhead = serde_json::to_string(question).unwrap().len()
                + 3
                + usize::from(!out["answers"].as_object().unwrap().is_empty());
            let Some(left) = remaining.checked_sub(overhead) else {
                truncated = true;
                break;
            };
            remaining = left;
            out["answers"][question] = json!(text_prefix(answer, &mut remaining, &mut truncated));
        }
        out["truncated"] = json!(truncated);
    }
    out
}

/// Resume the main wire at the last complete line. Stop hooks run before turn.ended,
/// so every status read checks for new durable evidence and reapplies the cached result.
fn wire_events(s: &mut Value) -> Vec<Value> {
    use std::{
        fs::OpenOptions,
        io::{BufRead, BufReader, Seek, SeekFrom},
        os::unix::fs::{MetadataExt, OpenOptionsExt},
    };
    s["kimi_status"] =
        json!({"interaction":null,"resolved":null,"questions":null,"command":null,"action":null});
    let Some(turn_started) = s["turn_started"].as_f64() else {
        return vec![];
    };
    // The wire's timestamps are milliseconds; a fast hook can return in the same millisecond.
    let since = (turn_started * 1000.0).floor() / 1000.0;
    s["reply"] = Value::Null;
    s["reply_t"] = Value::Null;
    let path = s["transcript_path"].as_str().unwrap_or("").to_owned();
    let Ok(mut file) = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(&path)
    else {
        return vec![];
    };
    let Some(meta) = file.metadata().ok().filter(|m| m.is_file()) else {
        return vec![];
    };
    let mut wire = serde_json::from_value::<WireCursor>(s["kimi_wire"].take())
        .ok()
        .filter(|w| {
            w.version == 2
                && w.path == path
                && w.dev == meta.dev()
                && w.ino == meta.ino()
                && w.len <= meta.len()
                && w.offset <= meta.len()
                && w.turn_started == turn_started
        })
        .unwrap_or_else(|| WireCursor {
            version: 2,
            path,
            dev: meta.dev(),
            ino: meta.ino(),
            turn_started,
            ..WireCursor::default()
        });
    wire.len = meta.len();
    if file.seek(SeekFrom::Start(wire.offset)).is_err() {
        return vec![];
    }
    let mut reader = BufReader::new(file);
    let mut line = Vec::new();
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line).unwrap_or(0) == 0 || line.last() != Some(&b'\n') {
            break;
        }
        wire.offset += line.len() as u64;
        // Avoid parsing request bodies, model context and tool output unrelated to state/text.
        let raw = String::from_utf8_lossy(&line);
        if ![
            "turn.prompt",
            "turn.ended",
            "content.part",
            "tool.call",
            "tool.result",
            "interaction.request",
            "interaction.resolved",
        ]
        .iter()
        .any(|kind| raw.contains(kind))
        {
            continue;
        }
        let Ok(e) = serde_json::from_slice::<Value>(&line) else {
            continue;
        };
        let t = e["time"].as_f64().unwrap_or(0.0) / 1000.0;
        if e["agentId"] != "main" {
            continue;
        }
        // Interaction lifetime is independent of a user turn. Replay older requests too.
        if e["type"] == "context.append_loop_event" {
            let part = &e["event"];
            if part["type"] == "tool.call"
                && part["name"] == "AskUserQuestion"
                && part["args"]["background"] == true
                && let Some(id) = part["toolCallId"].as_str()
            {
                wire.background_calls.push(id.into());
            }
        }
        if e["type"] == "interaction.request"
            && matches!(e["kind"].as_str(), Some("approval" | "question"))
            && e["id"].is_string()
        {
            let request = &e["request"];
            let background = request["toolCallId"]
                .as_str()
                .is_some_and(|id| wire.background_calls.iter().any(|c| c == id));
            wire.background_calls
                .retain(|id| request["toolCallId"] != *id);
            let mut dialog = json!({"id":e["id"],"kind":e["kind"],"at":t,
                "tool":if e["kind"] == "question" { json!("AskUserQuestion") } else { request["toolName"].clone() }});
            if e["kind"] == "question" {
                if let Some(mut questions) = crate::hooks::questions(&request["questions"]) {
                    if background {
                        questions["background"] = json!(true);
                        // Leave room for the additional flag without exceeding the R43 budget.
                        while questions.to_string().len() > 16 * 1024 {
                            questions["truncated"] = json!(true);
                            let items = questions["items"].as_array_mut().unwrap();
                            let item = items.last_mut().unwrap();
                            let options = item["options"].as_array_mut().unwrap();
                            if let Some(option) = options.last_mut() {
                                let key = if option["description"].as_str().unwrap().is_empty() {
                                    "label"
                                } else {
                                    "description"
                                };
                                let mut text = option[key].as_str().unwrap().to_owned();
                                if text.pop().is_some() {
                                    option[key] = json!(text);
                                } else {
                                    options.pop();
                                }
                            } else {
                                let key = if item["question"].as_str().unwrap().is_empty() {
                                    "header"
                                } else {
                                    "question"
                                };
                                let mut text = item[key].as_str().unwrap().to_owned();
                                if text.pop().is_some() {
                                    item[key] = json!(text);
                                } else {
                                    items.pop();
                                }
                            }
                        }
                    }
                    dialog["questions"] = questions;
                }
            } else {
                dialog["action"] = bounded_text(&request["action"]);
                if request["toolName"] == "Bash" && request["display"]["kind"] == "command" {
                    dialog["command"] = bounded_text(&request["display"]["command"]);
                }
            }
            wire.interactions.retain(|i| i["id"] != e["id"]);
            wire.interactions.push(dialog);
            wire.resolved = Value::Null;
        }
        if e["type"] == "interaction.resolved"
            && let Some(i) = wire.interactions.iter().position(|i| i["id"] == e["id"])
        {
            let dialog = wire.interactions.remove(i);
            wire.resolved = resolved(&dialog, &e, t);
        }
        if e["type"] == "turn.prompt" {
            wire.running = true;
            wire.prompt_at = t;
            if e["origin"]["kind"] == "user" {
                wire.resolved = Value::Null;
            }
        } else if e["type"] == "turn.ended" {
            wire.running = false;
        }
        if t < since {
            continue;
        }
        if e["type"] == "turn.prompt" {
            wire.turn = e["turnId"].clone();
            wire.step = Value::Null;
            wire.text.clear();
            wire.tool = Value::Null;
            wire.interaction = Value::Null;
            wire.state = Some(json!({"ev":"PreToolUse","t":t}));
        }
        if wire.turn.is_null() {
            continue;
        }
        match e["type"].as_str().unwrap_or("") {
            "context.append_loop_event" => {
                let part = &e["event"];
                let same_turn = part["turnId"]
                    .as_str()
                    .and_then(|id| id.parse::<u64>().ok())
                    .is_some_and(|id| wire.turn.as_u64() == Some(id))
                    || part["turnId"] == wire.turn;
                match part["type"].as_str().unwrap_or("") {
                    "content.part" if same_turn && part["part"]["type"] == "text" => {
                        if wire.step != part["step"] {
                            wire.text.clear();
                            wire.step = part["step"].clone();
                        }
                        wire.text
                            .push_str(part["part"]["text"].as_str().unwrap_or(""));
                    }
                    "tool.call" if same_turn => {
                        wire.tool = part["name"].clone();
                        wire.state = Some(json!({"ev":"PreToolUse","t":t}));
                    }
                    "tool.result" => {
                        wire.state = Some(json!({"ev":"PostToolUse","t":t}));
                    }
                    _ => {}
                }
            }
            "interaction.request"
                if e["request"]["turnId"] == wire.turn
                    && matches!(e["kind"].as_str(), Some("approval" | "question")) =>
            {
                wire.interaction = e["id"].clone();
                wire.tool = if e["kind"] == "question" {
                    json!("AskUserQuestion")
                } else {
                    e["request"]["toolName"].clone()
                };
                let mut state = json!({"ev":"PermissionRequest","t":t,"tool_name":wire.tool});
                if e["kind"] == "question"
                    && let Some(questions) = crate::hooks::questions(&e["request"]["questions"])
                {
                    state["questions"] = questions;
                }
                wire.state = Some(state);
            }
            "interaction.resolved" if e["id"] == wire.interaction => {
                wire.interaction = Value::Null;
                if wire.running {
                    wire.state = Some(json!({"ev":"PostToolUse","t":t}));
                }
            }
            "turn.ended" if e["turnId"] == wire.turn => {
                wire.state = match e["reason"].as_str() {
                    Some("completed") => {
                        Some(json!({"ev":"Stop","t":t,"last_assistant_message":wire.text}))
                    }
                    Some("cancelled") => Some(json!({"ev":"Interrupt","t":t})),
                    Some("failed" | "blocked") => Some(json!({"ev":"StopFailure","t":t})),
                    _ => wire.state,
                };
            }
            _ => {}
        }
    }
    s["last_tool"] = wire.tool.clone();
    if wire.resolved["at"].as_f64().is_some_and(|at| at < since) {
        wire.resolved = Value::Null;
    }
    let mut status = json!({"resolved":wire.resolved,"interaction":{"kind":null,"count":0,
        "running":wire.running || wire.prompt_at < since},"questions":null,"command":null,"action":null});
    if let Some(first) = wire.interactions.first() {
        // Kimi replaces same-kind UI, but queues different kinds behind the active owner.
        let front = wire
            .interactions
            .iter()
            .rev()
            .find(|i| i["kind"] == first["kind"])
            .unwrap();
        status["state"] = json!("blocked");
        status["state_started"] = front["at"].clone();
        status["last_tool"] = front["tool"].clone();
        status["questions"] = front["questions"].clone();
        status["command"] = front["command"].clone();
        status["action"] = front["action"].clone();
        status["interaction"] = json!({"kind":front["kind"],"count":wire.interactions.len(),
            "running":wire.running || wire.prompt_at < since});
    }
    s["kimi_status"] = status;
    s["kimi_wire"] = serde_json::to_value(&wire).unwrap();
    wire.state.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workdir_keys_match_kimi_23() {
        // Kimi's /[^a-z0-9._-]+/g replacement keeps allowed hyphens beside invalid runs.
        for (directory, expected) in [
            ("/tmp/foo - bar", "wd_foo---bar_d735df166c6d"),
            ("/tmp/a-(b)", "wd_a--b_15671396560d"),
            ("/tmp/my-项目x", "wd_my--x_3a26b02026a0"),
            ("/tmp/Hello.World_42", "wd_hello.world_42_32d7bd12d323"),
            ("/tmp/项目", "wd_workspace_8b9f6db86618"),
            ("/tmp/--a!? b--", "wd_a-b_42e0cc8fd133"),
        ] {
            assert_eq!(workdir_key(Path::new(directory)), expected, "{directory}");
        }
    }
}
