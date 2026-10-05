mod after;
mod attach;
mod cli;
mod environment;
mod events;
mod hooks;
mod pen;
mod skills;
mod state;
mod terminal;
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
fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
pub fn run() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "__pen-probe") {
        println!("{}", json!({"ok":true,"schema":pen::upgrade::SCHEMA}));
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
        return after::worker(args.get(1).map(String::as_str).unwrap_or(""));
    }
    if args.first().is_some_and(|s| s == "__pen") {
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
