//! `ranch`: shared commands for the Saddle and paddock front ends. Every command prints JSON
//! on stdout; usage errors exit 2.
use serde_json::{Value, json};
use std::{io::Write, process::ExitCode};

const USAGE: &str = "Usage:
  ranch dispatch route < SUMMARY
  ranch dispatch install-skills [--target all|claude|codex] [--dry-run] [--yes]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (code, out) = match args.as_slice() {
        [] | ["--help" | "-h"] => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        ["dispatch", "route"] => {
            let result = ranch_dispatch::route(&mut std::io::stdin().lock());
            (result.exit_code, result.stdout)
        }
        ["dispatch", "install-skills", rest @ ..] => install_skills(rest),
        _ => usage("unknown command"),
    };
    let mut stdout = std::io::stdout().lock();
    if stdout.write_all(&out).and_then(|_| stdout.flush()).is_err() {
        return ExitCode::from(1);
    }
    ExitCode::from(code)
}

fn install_skills(args: &[&str]) -> (u8, Vec<u8>) {
    let (mut target, mut dry, mut yes) = ("all", false, false);
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match *arg {
            "--dry-run" => dry = true,
            "--yes" => yes = true,
            "--target" => match rest.next() {
                Some(value) => target = value,
                None => return usage("--target needs a value"),
            },
            _ => return usage("unknown install-skills argument"),
        }
    }
    match ranch_dispatch::install_skills(target, dry, yes) {
        Ok(value) => (0, line(&value)),
        Err((code, value)) => (code, line(&value)),
    }
}

fn usage(message: &str) -> (u8, Vec<u8>) {
    eprintln!("{USAGE}");
    (
        2,
        line(&json!({"ok":false,"error":"usage","message":message})),
    )
}

fn line(value: &Value) -> Vec<u8> {
    let mut out = serde_json::to_vec(value).expect("JSON value");
    out.push(b'\n');
    out
}
