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
    for c in base.chars() {
        if c.is_ascii_alphanumeric() || "._-".contains(c) {
            slug.push(c);
        } else if !slug.ends_with('-') {
            slug.push('-');
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

/// Stream the main wire, retaining only this turn's text and latest state. Stop hooks run
/// before turn.ended is written, so every status read rechecks the durable evidence.
fn wire_events(s: &mut Value) -> Vec<Value> {
    use std::{
        fs::OpenOptions,
        io::{BufRead, BufReader},
        os::unix::fs::OpenOptionsExt,
    };
    let Some(since) = s["turn_started"].as_f64() else {
        return vec![];
    };
    // The wire's timestamps are milliseconds; a fast hook can return in the same millisecond.
    let since = (since * 1000.0).floor() / 1000.0;
    s["reply"] = Value::Null;
    s["reply_t"] = Value::Null;
    let Some(file) = s["transcript_path"]
        .as_str()
        .and_then(|p| {
            OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(p)
                .ok()
        })
        .filter(|f| f.metadata().is_ok_and(|m| m.is_file()))
    else {
        return vec![];
    };
    let mut reader = BufReader::new(file);
    let (mut turn, mut tool, mut interaction) = (Value::Null, Value::Null, Value::Null);
    let (mut text, mut line) = (String::new(), Vec::new());
    let mut state = None;
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line).unwrap_or(0) == 0 || line.last() != Some(&b'\n') {
            break;
        }
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
        if e["agentId"] != "main" || t < since {
            continue;
        }
        if e["type"] == "turn.prompt" {
            turn = e["turnId"].clone();
            text.clear();
            tool = Value::Null;
            interaction = Value::Null;
            state = Some(json!({"ev":"PreToolUse","t":t}));
        }
        if turn.is_null() {
            continue;
        }
        match e["type"].as_str().unwrap_or("") {
            "context.append_loop_event" => {
                let part = &e["event"];
                let same_turn = part["turnId"]
                    .as_str()
                    .and_then(|id| id.parse::<u64>().ok())
                    .is_some_and(|id| turn.as_u64() == Some(id))
                    || part["turnId"] == turn;
                match part["type"].as_str().unwrap_or("") {
                    "content.part" if same_turn && part["part"]["type"] == "text" => {
                        text.push_str(part["part"]["text"].as_str().unwrap_or(""));
                    }
                    "tool.call" if same_turn => {
                        tool = part["name"].clone();
                        state = Some(json!({"ev":"PreToolUse","t":t}));
                    }
                    "tool.result" => {
                        state = Some(json!({"ev":"PostToolUse","t":t}));
                    }
                    _ => {}
                }
            }
            "interaction.request"
                if e["request"]["turnId"] == turn
                    && matches!(e["kind"].as_str(), Some("approval" | "question")) =>
            {
                interaction = e["id"].clone();
                tool = if e["kind"] == "question" {
                    json!("AskUserQuestion")
                } else {
                    e["request"]["toolName"].clone()
                };
                state = Some(json!({"ev":"PermissionRequest","t":t}));
            }
            "interaction.resolved" if e["id"] == interaction => {
                interaction = Value::Null;
                state = Some(json!({"ev":"PostToolUse","t":t}));
            }
            "turn.ended" if e["turnId"] == turn => {
                state = match e["reason"].as_str() {
                    Some("completed") => {
                        Some(json!({"ev":"Stop","t":t,"last_assistant_message":text}))
                    }
                    Some("cancelled") => Some(json!({"ev":"Interrupt","t":t})),
                    Some("failed" | "blocked") => Some(json!({"ev":"StopFailure","t":t})),
                    _ => state,
                };
            }
            _ => {}
        }
    }
    s["last_tool"] = tool;
    state.into_iter().collect()
}
