//! pi. From Saddle `crates/corral-core/src/hooks.rs` and `events/raw.rs` at commit `a31dea2`
//! (split per agent in R3); comments from the Python corral `src/corral/agents/pi.py`.
use super::{Adapter, Launch};
use crate::Result;
use serde_json::{Value, json};

pub struct Pi;
impl Adapter for Pi {
    fn kind(&self) -> &'static str {
        "pi"
    }
    /// Loads the fact collector `hook_pi.ts` with `--extension`, alongside the user's own
    /// extensions; pi runs it in its own TypeScript runtime.
    fn hook_args(&self, launch: &Launch) -> Result<Vec<String>> {
        let path = launch.hook_file("hook_pi.ts", include_str!("../../resources/hook_pi.ts"))?;
        Ok(vec!["--extension".into(), path])
    }
    /// The first Ctrl-C only clears the input box; SIGHUP and SIGTERM both wind down normally.
    fn quit_steps(&self) -> Value {
        json!([{"signal":"HUP","wait":5},{"signal":"TERM","wait":3}])
    }
    fn raw_events(&self) -> bool {
        true
    }
    /// Only quitting ends the session; pi also shuts a session down to reload or switch it.
    fn raw_shutdown_ends(&self, reason: &Value) -> bool {
        reason.is_null() || reason == "quit"
    }
}
