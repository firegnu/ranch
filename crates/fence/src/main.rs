mod data;
mod rules;
mod runs;
mod runtime;
use serde_json::{Value, json};
use std::{env, path::PathBuf};

type Result<T> = std::result::Result<T, Value>;
fn error(kind: &str, message: impl ToString) -> Value {
    json!({"ok":false,"error":kind,"message":message.to_string()})
}
fn home() -> Result<PathBuf> {
    let path = env::var_os("FENCE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|p| PathBuf::from(p).join(".fence")))
        .ok_or_else(|| error("invalid_data", "HOME is not set"))?;
    std::path::absolute(path).map_err(|e| error("invalid_data", e))
}
fn dispatch(args: &[String]) -> Result<Value> {
    if args.first().is_some_and(|s| s == "run") {
        return runs::run(&args[1..]);
    }
    if args.first().is_some_and(|s| s == "denials") {
        return runs::denials(&args[1..]);
    }
    if args.first().is_some_and(|s| s == "check") {
        let options = options(&args[1..], &["--profile", "--kind", "--cwd"])?;
        let profile = required(&options, "--profile")?;
        let cwd = options
            .get("--cwd")
            .map(PathBuf::from)
            .unwrap_or(env::current_dir().map_err(|e| error("bad_cwd", e))?);
        let kind = options.get("--kind").map(String::as_str);
        let resolved = rules::resolve(&home()?, profile, kind, &cwd, None, &[], &[])?;
        let mut value = resolved.value;
        if let Some(kind) = kind {
            runtime::program(kind)?;
        }
        value["runtime"] = runtime::check()?;
        return Ok(value);
    }
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["profiles"] => data::profiles(&home()?),
        ["--version"] => Ok(
            json!({"ok":true,"version":env!("CARGO_PKG_VERSION"),"contract":"1","srt":"0.0.80","node":runtime::node_version()}),
        ),
        _ => Err(error(
            "usage",
            "expected --version, profiles, check, run or denials",
        )),
    }
}
fn options(
    args: &[String],
    allowed: &[&str],
) -> Result<std::collections::BTreeMap<String, String>> {
    let mut out = std::collections::BTreeMap::new();
    let mut args = args.iter();
    while let Some(key) = args.next() {
        if !allowed.contains(&key.as_str()) {
            return Err(error("usage", format!("unknown option {key}")));
        }
        let value = args
            .next()
            .ok_or_else(|| error("usage", format!("missing value for {key}")))?;
        if out.insert(key.clone(), value.clone()).is_some() {
            return Err(error("usage", format!("repeated option {key}")));
        }
    }
    Ok(out)
}
fn required<'a>(
    options: &'a std::collections::BTreeMap<String, String>,
    key: &str,
) -> Result<&'a str> {
    options
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| error("usage", format!("missing {key}")))
}
fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    let (value, code) = match dispatch(&args) {
        Ok(v) => (v, 0),
        Err(v) => (v, 1),
    };
    if args.first().is_some_and(|s| s == "run") && code != 0 {
        eprintln!("{value}");
        std::process::exit(125);
    }
    println!("{value}");
    std::process::exit(code);
}
