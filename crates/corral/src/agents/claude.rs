//! Claude Code. From Saddle `crates/corral-core/src/hooks.rs` at commit `a31dea2` (split per agent
//! in R3); comments from the Python corral `src/corral/agents/claude.py`.
use super::{Adapter, Launch};
use crate::Result;
use serde_json::{Value, json};

pub struct Claude;
impl Adapter for Claude {
    fn kind(&self) -> &'static str {
        "claude"
    }
    /// Hooks go in through `--settings` and apply together with those in the user's own settings.
    fn hook_args(&self, launch: &Launch) -> Result<Vec<String>> {
        let mut hooks = json!({});
        for ev in [
            "SessionStart",
            "UserPromptSubmit",
            "PreToolUse",
            "PostToolUse",
            "PermissionRequest",
            "Notification",
            "Stop",
            "StopFailure",
            "SessionEnd",
        ] {
            let mut h = json!({"hooks":[{"type":"command","command":launch.hook_command(ev),"timeout":10}]});
            if ["PreToolUse", "PostToolUse", "PermissionRequest"].contains(&ev) {
                h["matcher"] = json!("*")
            }
            hooks[ev] = json!([h]);
        }
        Ok(vec![
            "--settings".into(),
            json!({"hooks":hooks}).to_string(),
        ])
    }
    /// Measured: exits normally about a second after SIGHUP.
    fn quit_steps(&self) -> Value {
        json!([{"signal":"HUP","wait":5},{"signal":"TERM","wait":3}])
    }
    fn skill_targets(&self) -> &'static [&'static str] {
        &["claude"]
    }
}
