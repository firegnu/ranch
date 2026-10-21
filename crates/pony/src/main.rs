mod chats;
mod core;

use serde_json::{Value, json};
use std::{env, path::PathBuf};

fn home() -> core::Result<PathBuf> {
    let path = env::var_os("PONY_HOME")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME")
                .filter(|p| !p.is_empty())
                .map(|p| PathBuf::from(p).join(".pony"))
        })
        .ok_or_else(|| core::Error::new("invalid_data", "HOME is not set"))?;
    // Remove trailing separators and '.' without following directory symlinks.
    let path: PathBuf = path.components().collect();
    std::path::absolute(path).map_err(|e| core::Error::new("invalid_data", e.to_string()))
}

fn dispatch(args: &[String]) -> (Value, i32) {
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["--version"] => (
            json!({"ok": true, "version": env!("CARGO_PKG_VERSION"), "contract": "1"}),
            0,
        ),
        [action @ ("status" | "run")] => match home().and_then(|home| {
            if *action == "status" {
                Ok(core::status::read(&home))
            } else {
                core::runtime::run(&home).map(|()| json!({"ok": true}))
            }
        }) {
            Ok(value) => (value, 0),
            Err(error) => (
                json!({"ok": false, "error": error.code, "message": error.message}),
                1,
            ),
        },
        _ => (
            json!({"ok": false, "error": "usage", "message": "expected --version, status or run"}),
            2,
        ),
    }
}

fn main() {
    let (value, code) = dispatch(&env::args().skip(1).collect::<Vec<_>>());
    println!("{value}");
    std::process::exit(code);
}
