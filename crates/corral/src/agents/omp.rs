//! omp (Oh My Pi, a fork of pi). From Saddle `crates/corral-core/src/hooks.rs` and
//! `events/raw.rs` at commit `a31dea2` (split per agent in R3); comments from the Python corral
//! `src/corral/agents/omp.py`.
use super::{Adapter, Launch};
use crate::Result;
use serde_json::{Value, json};

pub struct Omp;
impl Adapter for Omp {
    fn kind(&self) -> &'static str {
        "omp"
    }
    /// Like pi, loads its collector with `--extension`. omp's events share pi's origin but differ
    /// (no `agent_settled`, different approval events, sub-sessions), so it has its own collector.
    fn hook_args(&self, launch: &Launch) -> Result<Vec<String>> {
        let path = launch.hook_file("hook_omp.ts", include_str!("../../resources/hook_omp.ts"))?;
        Ok(vec!["--extension".into(), path])
    }
    fn quit_steps(&self) -> Value {
        json!([{"signal":"HUP","wait":5},{"signal":"TERM","wait":3}])
    }
    fn raw_events(&self) -> bool {
        true
    }
    /// omp also reports its sub-sessions; only the main session, the one with a UI, is the agent's.
    fn raw_ignored(&self, event: &Value) -> bool {
        event["has_ui"] != true
    }
    /// omp's `ask` tool puts a question to the user.
    fn raw_asks(&self, tool: &Value) -> bool {
        tool == "ask"
    }
}
