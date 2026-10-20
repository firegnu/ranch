//! `ranch`: shared commands for the Saddle and paddock front ends. Every command prints JSON
//! on stdout; usage errors exit 2.
use serde_json::{Value, json};
use std::{io::Write, process::ExitCode};

const USAGE: &str = "Usage:
  ranch dispatch route < SUMMARY
  ranch dispatch cell <NAME>
  ranch dispatch table
  ranch dispatch edit <NAME> [--agents <JSON>] [--model <MODEL>] [--effort <EFFORT>] [--cross-review <CELL>] [--rework <CELL>] [--description <TEXT>] [--if-version <VERSION>]
  ranch dispatch reset <NAME> [--if-version <VERSION>]
  ranch dispatch install-skills [--target all|claude|codex|gemini] [--dry-run] [--yes]";

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
        ["dispatch", "cell", name] => table(Some(name)),
        ["dispatch", "table"] => table(None),
        ["dispatch", command @ ("edit" | "reset"), name, rest @ ..] => {
            edit_table(name, rest, *command == "reset")
        }
        _ => usage("unknown command"),
    };
    let mut stdout = std::io::stdout().lock();
    if stdout.write_all(&out).and_then(|_| stdout.flush()).is_err() {
        return ExitCode::from(1);
    }
    ExitCode::from(code)
}

fn table(name: Option<&str>) -> (u8, Vec<u8>) {
    match ranch_dispatch::read_table(name) {
        Ok(value) => (0, line(&value)),
        Err((code, value)) => (code, line(&value)),
    }
}

fn edit_table(name: &str, args: &[&str], reset: bool) -> (u8, Vec<u8>) {
    let mut patch = ranch_dispatch::CellPatch::default();
    let mut expected_version = None;
    let mut seen = std::collections::BTreeSet::new();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if !seen.insert(arg) {
            return usage("duplicate edit argument");
        }
        let Some(value) = rest.next() else {
            return usage("edit argument needs a value");
        };
        if *arg == "--if-version" {
            expected_version = Some(*value);
            continue;
        }
        if reset {
            return usage("reset does not accept field arguments");
        }
        match *arg {
            "--agents" => match serde_json::from_str::<Vec<String>>(value) {
                Ok(agents) => patch.agents = Some(agents),
                Err(_) => return usage("--agents needs a JSON string array"),
            },
            "--model" => patch.model = Some((*value).to_owned()),
            "--effort" => patch.effort = Some((*value).to_owned()),
            "--cross-review" => patch.cross_review = Some((*value).to_owned()),
            "--rework" => patch.rework = Some((*value).to_owned()),
            "--description" => patch.description = Some((*value).to_owned()),
            _ => return usage("unknown edit argument"),
        }
    }
    if seen.len() == usize::from(expected_version.is_some()) && !reset {
        return usage("edit needs at least one field");
    }
    match ranch_dispatch::edit_table(
        name,
        if reset { None } else { Some(patch) },
        expected_version,
    ) {
        Ok(value) => (0, line(&value)),
        Err((code, value)) => (code, line(&value)),
    }
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
