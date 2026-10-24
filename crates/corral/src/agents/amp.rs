//! amp (Amp's command-line agent). New in ranch (R6); the research is in
//! `docs/调研/R5-agy与amp接入、全局安装和读屏兜底.md`.
use super::{Adapter, Global, Launch};
use crate::Result;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub struct Amp;
impl Adapter for Amp {
    fn kind(&self) -> &'static str {
        "amp"
    }
    /// amp has no way to load a plugin for one run; its hooks come from the installed plugin.
    fn hook_args(&self, _launch: &Launch) -> Result<Vec<String>> {
        Ok(vec![])
    }
    fn quit_steps(&self) -> Value {
        json!([{"signal":"HUP","wait":3},{"signal":"TERM","wait":3}])
    }
    /// amp reads Claude Code's and the Agent Skills locations, besides its own.
    fn skill_targets(&self) -> &'static [&'static str] {
        &["claude", "codex"]
    }
    /// Interactive amp takes no first prompt on its command line.
    fn prompt(&self) -> bool {
        false
    }
    /// amp's plugin API keeps saying `running` while a command waits for approval, but amp puts
    /// `◆ Input needed: <thread title>` in the terminal title.
    fn title_blocked(&self, title: &str) -> bool {
        title.starts_with("◆ Input needed")
    }
    /// A system plugin: `$XDG_CONFIG_HOME/amp/plugins`, by default under `~/.config`.
    fn global(&self) -> Option<Global> {
        Some(Global {
            dir: config,
            path: "plugins/corral.ts",
            files: &[("", include_str!("../../resources/amp_plugin.ts"))],
            enabled: None,
            helper: false,
            block: None,
        })
    }
}

/// amp's configuration directory.
fn config(home: &Path) -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.join(".config"))
        .join("amp")
}
