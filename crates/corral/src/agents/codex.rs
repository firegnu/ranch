//! Codex. From Saddle `crates/corral-core/src/hooks.rs` at commit `a31dea2` (split per agent in
//! R3); comments from the Python corral `src/corral/agents/codex.py`.
use super::{Adapter, Launch};
use crate::Result;
use serde_json::{Value, json};

pub struct Codex;
impl Adapter for Codex {
    fn kind(&self) -> &'static str {
        "codex"
    }
    /// Hooks go in through `-c`. `--dangerously-bypass-hook-trust` is required: without it Codex
    /// asks to review the hooks at start, and trusting them writes their hashes into the user's
    /// `config.toml`. Codex's `Interrupt` hook is the reason the shared reader knows that event.
    fn hook_args(&self, launch: &Launch) -> Result<Vec<String>> {
        let mut out = Vec::new();
        for ev in [
            "SessionStart",
            "UserPromptSubmit",
            "PreToolUse",
            "PostToolUse",
            "PermissionRequest",
            "Stop",
            "Interrupt",
        ] {
            // Codex cuts the Interrupt hook's timeout to 3 seconds and warns about larger values.
            let timeout = if ev == "Interrupt" { 3 } else { 10 };
            // A JSON string is also a valid TOML basic string.
            out.extend([
                "-c".into(),
                format!(
                    "hooks.{ev}=[{{hooks=[{{type=\"command\",command={},timeout={timeout}}}]}}]",
                    json!(launch.hook_command(ev))
                ),
            ]);
        }
        out.push("--dangerously-bypass-hook-trust".into());
        Ok(out)
    }
    /// Measured: ignores SIGHUP. Two Ctrl-C show "Shutting down..." and exit normally, but the
    /// wind-down is long, grows with the session (7.6 s fresh, 27.7 s after three turns) and prints
    /// nothing to judge by, so wait the longest step a pen allows (60 s) before terminating.
    fn quit_steps(&self) -> Value {
        json!([{"keys":"Aw==","wait":0.3},{"keys":"Aw==","wait":60},{"signal":"TERM","wait":3}])
    }
    fn skill_targets(&self) -> &'static [&'static str] {
        &["codex"]
    }
}
