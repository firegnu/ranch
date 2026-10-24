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
}

/// Resume the main wire at the last complete line. Stop hooks run before turn.ended,
/// so every status read checks for new durable evidence and reapplies the cached result.
fn wire_events(s: &mut Value) -> Vec<Value> {
    use std::{
        fs::OpenOptions,
        io::{BufRead, BufReader, Seek, SeekFrom},
        os::unix::fs::{MetadataExt, OpenOptionsExt},
    };
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
            w.path == path
                && w.dev == meta.dev()
                && w.ino == meta.ino()
                && w.len <= meta.len()
                && w.offset <= meta.len()
                && w.turn_started == turn_started
        })
        .unwrap_or_else(|| WireCursor {
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
        .any(|kind| {
            line.windows(kind.len())
                .any(|bytes| bytes == kind.as_bytes())
        }) {
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
                wire.state = Some(json!({"ev":"PermissionRequest","t":t}));
            }
            "interaction.resolved" if e["id"] == wire.interaction => {
                wire.interaction = Value::Null;
                wire.state = Some(json!({"ev":"PostToolUse","t":t}));
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
