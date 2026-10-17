// From Saddle `crates/corral-core/src/lib.rs` at commit `a31dea2`; ranch has changed it since.
#[cfg(test)]
mod adapter_snapshot;
mod after;
mod agents;
mod attach;
mod cli;
mod environment;
mod events;
mod freeze;
mod hooks;
mod integration;
mod pen;
mod screen;
mod skills;
mod state;
mod terminal;
mod trust;
mod upgrade;
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

type Result<T> = std::result::Result<T, Error>;
#[derive(Debug)]
struct Error {
    code: i32,
    value: Value,
}
impl Error {
    fn new(code: i32, kind: &str, message: impl ToString) -> Self {
        Self {
            code,
            value: json!({"ok":false,"error":kind,"message":message.to_string()}),
        }
    }
    fn with(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.value[key] = value.into();
        self
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::new(1, "error", e)
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::new(1, "error", e)
    }
}
// On macOS current_exe may preserve a public symlink. Workers and hooks must not.
fn executable() -> std::io::Result<std::path::PathBuf> {
    std::fs::canonicalize(std::env::current_exe()?)
}
/// For a process that stays in the background: closes every descriptor but 0, 1 and 2. All of them
/// were inherited, since exec closed the close-on-exec ones; a caller can leak one by accident (on
/// macOS, Rust sets close-on-exec on a new pipe in a second step, and another thread may spawn in
/// between), and holding a pipe it waits on would keep that caller waiting (R9).
fn close_inherited() {
    let Ok(dir) = std::fs::read_dir("/dev/fd") else {
        return;
    };
    let fds: Vec<i32> = dir
        .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .filter(|&fd| fd > 2)
        .collect();
    // The listing's own descriptor is among them and already closed.
    for fd in fds {
        unsafe { libc::close(fd) };
    }
}
fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
pub fn run() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "__pen-probe") {
        println!(
            "{}",
            json!({"ok":true,"schema":pen::upgrade::SCHEMA,"pause":1})
        );
        return 0;
    }
    if args.first().is_some_and(|s| s == "__pen-resume") {
        return pen::upgrade::resume(&args[1..]);
    }
    if args.first().is_some_and(|s| s == "__hook") {
        hooks::run(args.get(1).map(String::as_str).unwrap_or(""));
        return 0;
    }
    if args.first().is_some_and(|s| s == "__after") {
        close_inherited();
        return after::worker(args.get(1).map(String::as_str).unwrap_or(""));
    }
    if args.first().is_some_and(|s| s == "__pen") {
        close_inherited();
        return pen::worker();
    }
    match cli::run(&args) {
        Ok(value) => {
            if let Some(v) = value {
                println!("{v}");
            }
            0
        }
        Err(e) => {
            if !e.value.is_null() {
                println!("{}", e.value);
            }
            e.code
        }
    }
}
