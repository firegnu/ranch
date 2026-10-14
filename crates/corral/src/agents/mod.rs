//! Agent adapters: one file per agent kind, registered in `ADAPTERS`. Shared code asks the
//! registry and never names a kind itself. The structure follows the Python corral
//! (`src/corral/agents/` at `6923da1`); the behaviour moved here in R3 from Saddle
//! `crates/corral-core/src/hooks.rs` and `events/raw.rs` at commit `a31dea2`.
//!
//! To add an agent: write `<kind>.rs` with a type implementing `Adapter`, and add it to
//! `ADAPTERS`. A file the adapter writes into the agent's state directory must also be listed in
//! `state::FILES` (names and cleanup depend on it; the tests check this).
//!
//! Hooks only apply to the agent corral starts: they are passed as start-up arguments or
//! loaded from the agent's state directory. An agent that cannot load hooks for one run declares
//! a `Global` file instead, which `corral integration install` writes only with the user's
//! consent; that file must do nothing unless `CORRAL_KIND` names its agent.
//!
//! An agent whose hooks cannot report that it waits for the user can say how its terminal shows
//! it: by its title, or by screen rules, for which its pen keeps a screen (`screen.rs`).
use crate::Result;
use crate::screen::Rule;
use serde_json::{Value, json};
use std::{
    borrow::Cow,
    fs::OpenOptions,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

mod agy;
mod amp;
mod claude;
mod codex;
mod omp;
mod pi;

/// Registration order is the order `integration status` lists agents in.
static ADAPTERS: &[&dyn Adapter] = &[
    &claude::Claude,
    &codex::Codex,
    &pi::Pi,
    &omp::Omp,
    &amp::Amp,
    &agy::Agy,
];

pub(crate) trait Adapter: Sync {
    /// The program's file name that selects this adapter; also recorded as the agent's `kind`.
    fn kind(&self) -> &'static str;
    /// Arguments placed right after the program to connect corral's hooks for this run.
    fn hook_args(&self, launch: &Launch) -> Result<Vec<String>>;
    /// Steps the pen follows to stop the agent (it always ends with SIGKILL).
    fn quit_steps(&self) -> Value;
    /// Which of corral's skill places (`skills::TARGETS`) this agent reads skills from.
    fn skill_targets(&self) -> &'static [&'static str] {
        &[]
    }
    /// Whether `corral start --prompt` can pass a first prompt on the command line.
    fn prompt(&self) -> bool {
        true
    }
    /// The arguments that pass the first prompt, after the user's own: by default after `--`, so
    /// options taking several values cannot swallow it and it may start with `-`.
    fn prompt_args(&self, prompt: &str) -> Vec<String> {
        vec!["--".into(), prompt.into()]
    }
    /// The events to record for `event` of a hook installed globally (`__hook <kind>:<event>`),
    /// given the hook's input; `hooks` adds the fields every event has.
    fn hook(&self, _event: &str, _input: &Value) -> Vec<Value> {
        vec![]
    }
    /// Whether the terminal title says the agent waits for the user while its hooks say it works or
    /// is idle; for agents whose hooks cannot report that.
    fn title_blocked(&self, _title: &str) -> bool {
        false
    }
    /// What on the screen shows the agent waits for the user, for agents whose hooks cannot report
    /// that. The pen keeps a screen only for an agent with rules.
    fn screen_rules(&self) -> &'static [Rule] {
        &[]
    }
    /// The file this agent needs in its global config to report to corral, if any.
    fn global(&self) -> Option<Global> {
        None
    }
    /// The agent's hook writes version 2 raw events (`"adapter": kind`) for `events::raw`.
    fn raw_events(&self) -> bool {
        false
    }
    /// A raw event to drop before it touches any state.
    fn raw_ignored(&self, _event: &Value) -> bool {
        false
    }
    /// A started tool that asks the user a question, so the agent waits for an answer.
    fn raw_asks(&self, _tool: &Value) -> bool {
        false
    }
    /// Whether `session_shutdown` with this reason ends the session.
    fn raw_shutdown_ends(&self, _reason: &Value) -> bool {
        true
    }
}

/// A hook file, or a directory of them, installed into an agent's own config directory, because
/// the agent cannot load hooks for a single run.
pub(crate) struct Global {
    /// The agent's config directory, given HOME. corral leaves its files alone if anything from a
    /// file up to this directory is a symbolic link.
    pub dir: fn(&Path) -> PathBuf,
    /// What corral installs inside `dir`: one file, or a directory holding only `files`.
    pub path: &'static str,
    /// Each file's path inside `path` (empty when `path` is the file) and what corral writes there;
    /// every file names its version as `CORRAL_INTEGRATION = <n>`. Installed in this order.
    pub files: &'static [(&'static str, &'static str)],
    /// Whether the agent has the installed hooks switched on, given HOME, when it says so.
    pub enabled: Option<fn(&Path) -> Option<bool>>,
}

/// What an adapter needs to build its hook arguments.
pub(crate) struct Launch<'a> {
    helper: String,
    dir: &'a Path,
}
impl Launch<'_> {
    /// Shell command that records `event` through the namespace's stable helper entry.
    pub fn hook_command(&self, event: &str) -> String {
        format!("{} __hook {event}", self.helper)
    }
    /// Writes a hook file into the agent's state directory and returns its path.
    pub fn hook_file(&self, name: &str, body: &str) -> Result<String> {
        let path = self.dir.join(name);
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;
        f.write_all(body.as_bytes())?;
        Ok(path.to_string_lossy().into_owned())
    }
}

pub fn find(kind: &str) -> Option<&'static dyn Adapter> {
    ADAPTERS.iter().copied().find(|a| a.kind() == kind)
}
pub fn all() -> impl Iterator<Item = &'static dyn Adapter> {
    ADAPTERS.iter().copied()
}
/// The agent kind of a program: its file name.
pub fn kind(program: &str) -> Cow<'_, str> {
    Path::new(program)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
}
/// Whether the pen keeps a screen for this kind.
pub fn reads_screen(kind: &str) -> bool {
    find(kind).is_some_and(|a| !a.screen_rules().is_empty())
}
/// Whether the agent waits for the user although its hooks say it works or is idle (`state`),
/// going by its terminal title and, from pens that keep one, its screen.
pub fn waits_on_terminal(a: &dyn Adapter, state: &str, title: &str, screen: Option<&str>) -> bool {
    matches!(state, "working" | "idle")
        && (a.title_blocked(title)
            || screen.is_some_and(|s| crate::screen::matches(a.screen_rules(), s)))
}
pub fn known(kind: &str) -> bool {
    find(kind).is_some()
}
/// Extra environment for an agent whose hooks are a global file: they act only when
/// `CORRAL_KIND` names their agent and the agent's parent is the pen `CORRAL_PEN_PID` (an agent
/// started inside it inherits the rest), and report the agent's directory as `CORRAL_CWD`.
/// Called in the pen.
pub fn env(program: &str, cwd: &str) -> Vec<(String, String)> {
    match find(&kind(program)).filter(|a| a.global().is_some()) {
        Some(a) => vec![
            ("CORRAL_KIND".into(), a.kind().into()),
            ("CORRAL_CWD".into(), cwd.into()),
            ("CORRAL_PEN_PID".into(), std::process::id().to_string()),
        ],
        None => vec![],
    }
}
pub fn quit_steps(kind: &str) -> Value {
    match find(kind) {
        Some(a) => a.quit_steps(),
        // An unknown program: hang up, then terminate.
        None => json!([{"signal":"HUP","wait":3},{"signal":"TERM","wait":3}]),
    }
}
/// The command line the pen starts: the program, its hook arguments, the user's arguments and
/// the first prompt.
pub fn argv(argv: &[String], dir: &Path, prompt: Option<&str>) -> Result<Vec<String>> {
    let mut out = vec![argv[0].clone()];
    let exe = crate::hooks::helper(&crate::executable()?, false)?;
    let helper = shell_words::quote(
        exe.to_str()
            .ok_or_else(|| crate::Error::new(1, "error", "non UTF-8 helper path"))?,
    )
    .into_owned();
    if let Some(adapter) = find(&kind(&argv[0])) {
        out.extend(adapter.hook_args(&Launch { helper, dir })?);
    }
    out.extend_from_slice(&argv[1..]);
    if let Some(p) = prompt {
        match find(&kind(&argv[0])) {
            Some(a) => out.extend(a.prompt_args(p)),
            None => out.extend(["--".into(), p.into()]),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_are_unique_and_files_written_are_internal() {
        let dir = tempfile::tempdir().unwrap();
        let launch = Launch {
            helper: "helper".into(),
            dir: dir.path(),
        };
        let mut kinds = Vec::new();
        for a in all() {
            assert!(!kinds.contains(&a.kind()), "{} registered twice", a.kind());
            kinds.push(a.kind());
            for t in a.skill_targets() {
                assert!(
                    crate::skills::TARGETS.contains(t),
                    "{}: unknown skill target {t}",
                    a.kind()
                );
            }
            a.hook_args(&launch).unwrap();
        }
        for entry in std::fs::read_dir(dir.path()).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            assert!(
                crate::state::FILES.contains(&name.as_str()),
                "{name} is missing from state::FILES"
            );
        }
    }

    #[test]
    fn the_terminal_turns_working_or_idle_into_blocked_only_where_the_adapter_says_so() {
        struct Screened;
        impl Adapter for Screened {
            fn kind(&self) -> &'static str {
                "screened"
            }
            fn hook_args(&self, _launch: &Launch) -> Result<Vec<String>> {
                Ok(vec![])
            }
            fn quit_steps(&self) -> Value {
                json!([])
            }
            fn screen_rules(&self) -> &'static [Rule] {
                &[Rule {
                    rows: 1,
                    all: &["Allow?"],
                    none: &[],
                }]
            }
        }
        let asking = Some("work\nAllow?");
        for state in ["working", "idle"] {
            assert!(waits_on_terminal(&Screened, state, "", asking));
            assert!(!waits_on_terminal(
                &Screened,
                state,
                "",
                Some("Allow?\nwork")
            ));
            // A pen without a screen.
            assert!(!waits_on_terminal(&Screened, state, "", None));
        }
        for state in ["blocked", "starting", "unknown", "stopped"] {
            assert!(!waits_on_terminal(&Screened, state, "", asking));
        }
        // Only agy reads its screen (R8), only amp its title.
        for a in all() {
            assert_eq!(reads_screen(a.kind()), a.kind() == "agy", "{}", a.kind());
            assert!(!waits_on_terminal(a, "working", "", asking), "{}", a.kind());
            assert_eq!(
                waits_on_terminal(a, "working", "◆ Input needed: x", None),
                a.kind() == "amp"
            );
        }
        assert!(!reads_screen("unknown-program"));
    }
}
