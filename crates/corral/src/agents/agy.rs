//! agy (Google Antigravity's command-line agent). New in ranch (R8); the research is in
//! `docs/调研/R5-agy与amp接入、全局安装和读屏兜底.md`, what real runs showed in
//! `docs/任务/R8-接入agy.md`.
use super::{Adapter, Global, Launch};
use crate::Result;
use crate::screen::Rule;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub struct Agy;
impl Adapter for Agy {
    fn kind(&self) -> &'static str {
        "agy"
    }
    /// agy has no way to load hooks for one run; they come from the installed plugin.
    fn hook_args(&self, _launch: &Launch) -> Result<Vec<String>> {
        Ok(vec![])
    }
    fn quit_steps(&self) -> Value {
        json!([{"signal":"HUP","wait":3},{"signal":"TERM","wait":3}])
    }
    /// agy reads global skills from its own config directory only.
    fn skill_targets(&self) -> &'static [&'static str] {
        &["gemini"]
    }
    /// One argument, so a prompt starting with `-` is not taken for an option.
    fn prompt_args(&self, prompt: &str) -> Vec<String> {
        vec![format!("--prompt-interactive={prompt}")]
    }
    /// No hook says agy asks for approval; its prompt ends with `↑/↓ Navigate · tab Amend · …`.
    /// Both key hints, not just the words, so a reply mentioning them does not match.
    fn screen_rules(&self) -> &'static [Rule] {
        &[Rule {
            rows: 4,
            all: &["↑/↓ Navigate", "tab Amend"],
            none: &[],
        }]
    }
    /// A plugin directory: `~/.gemini/config/plugins/corral/`.
    fn global(&self) -> Option<Global> {
        Some(Global {
            dir: config,
            path: "plugins/corral",
            files: &[
                ("hooks.json", include_str!("../../resources/agy_hooks.json")),
                (
                    "plugin.json",
                    include_str!("../../resources/agy_plugin.json"),
                ),
            ],
            enabled: Some(enabled),
            helper: true,
        })
    }
    /// agy's hook input has no prompt or reply; both come from its transcript.
    fn hook(&self, event: &str, input: &Value) -> Vec<Value> {
        let mut e = match event {
            "SessionStart" => json!({"ev":"SessionStart","source":"startup"}),
            // The first model call of a turn.
            "PreInvocation" if input["invocationNum"] == 0 => {
                let mut e = json!({"ev":"UserPromptSubmit"});
                if let Some(prompt) = turn(input).0 {
                    e["prompt"] = json!(prompt);
                }
                e
            }
            "PostToolUse" => match input["toolCall"]["name"].as_str() {
                Some(tool) => json!({"ev":"PostToolUse","tool_name":tool}),
                None => json!({"ev":"PostToolUse"}),
            },
            "Stop" => match input["terminationReason"].as_str() {
                Some("USER_CANCELED") => json!({"ev":"Interrupt"}),
                Some("ERROR") => json!({"ev":"StopFailure"}),
                _ => json!({"ev":"Stop","last_assistant_message":turn(input).1}),
            },
            _ => return vec![],
        };
        e["session_id"] = input["conversationId"].clone();
        e["has_transcript"] = json!(true);
        vec![e]
    }
}

/// agy's config directory.
fn config(home: &Path) -> PathBuf {
    home.join(".gemini/config")
}

/// Whether the user has left corral's plugin switched on (`agy plugin disable corral` turns it
/// off in agy's `config.json`); unknown if that file cannot be read.
fn enabled(home: &Path) -> Option<bool> {
    match std::fs::read(config(home).join("config.json")) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(true),
        Err(_) => None,
        Ok(bytes) => serde_json::from_slice::<Value>(&bytes)
            .ok()
            .map(|v| v["plugins"]["corral"]["enabled"].as_bool().unwrap_or(true)),
    }
}

/// The user's last input in the transcript the hook names, and the last text the model gave
/// since. Read line by line, parsing only the steps that can be either, so a long conversation
/// costs time but not memory.
fn turn(input: &Value) -> (Option<String>, Option<String>) {
    use std::io::BufRead;
    let Some(file) = input["transcriptPath"]
        .as_str()
        .and_then(|p| std::fs::File::open(p).ok())
    else {
        return (None, None);
    };
    let (mut prompt, mut reply) = (None, None);
    for line in std::io::BufReader::new(file).lines().map_while(|l| l.ok()) {
        if !line.contains("USER_INPUT") && !line.contains("PLANNER_RESPONSE") {
            continue;
        }
        let Ok(step) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let content = step["content"].as_str().unwrap_or("");
        match step["type"].as_str() {
            Some("USER_INPUT") => {
                prompt = Some(request(content).to_owned());
                reply = None;
            }
            Some("PLANNER_RESPONSE") if !content.is_empty() => reply = Some(content.to_owned()),
            _ => {}
        }
    }
    (prompt, reply)
}

/// What the user typed: a user step wraps it in `<USER_REQUEST>` beside metadata.
fn request(content: &str) -> &str {
    content
        .split_once("<USER_REQUEST>")
        .and_then(|(_, rest)| rest.split_once("</USER_REQUEST>"))
        .map_or(content, |(text, _)| text)
        .trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_approval_prompt_reads_as_waiting() {
        let rules = Agy.screen_rules();
        // As agy 1.3.5 showed it in the real run (R8).
        let approval = "Requesting permission for:\n   ls -la\nRun this command?\n> 1. Yes, run command\n  2. Yes, and always allow in this conversation for commands that start with 'ls'\n  3. Yes, and always allow for commands that start with 'ls' (Persist to settings.json)\n  4. No, cancel\n  ↑/↓ Navigate · tab Amend · ctrl+g edit/expand command\nesc to cancel                    Gemini 3.8 Flash · high\n\n";
        assert!(crate::screen::matches(rules, approval));
        // Dialogs that also navigate with the arrows wait for the user too, but not for approval:
        // trusting a folder and signing in.
        let trust = " Do you trust the files in this folder?\n > Yes\n   No\n ↑/↓ Navigate · enter Confirm\n";
        assert!(!crate::screen::matches(rules, trust));
        // Once answered, the prompt scrolls out of the last rows.
        let after = format!("{approval}ran ls\ntotal 8\nfile a\nfile b\n");
        assert!(!crate::screen::matches(rules, &after));
        // A reply that only mentions the words.
        let prose = "The API lets you Navigate and Amend records.\n> \n";
        assert!(!crate::screen::matches(rules, prose));
    }
}
