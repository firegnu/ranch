mod chats;
mod core;

use serde_json::{Value, json};
use std::{env, path::PathBuf};

fn home() -> PathBuf {
    env::var_os("PONY_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".pony"))
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
        ["status"] => (core::status::read(&home()), 0),
        ["run"] => match core::runtime::run(&home()) {
            Ok(()) => (json!({"ok": true}), 0),
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
