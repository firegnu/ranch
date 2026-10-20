use crate::{Result, error, home, options, required, rules, runtime};
use serde_json::{Value, json};
use std::{
    env, fs,
    io::Write,
    os::unix::{
        fs::{DirBuilderExt, OpenOptionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
fn io(e: std::io::Error) -> Value {
    error("invalid_data", e)
}
fn valid_instance(instance: &str) -> Result<()> {
    if instance.is_empty()
        || instance == "."
        || instance == ".."
        || !instance
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err(error(
            "usage",
            "instance must be a single filename (letters, digits, '.', '_' or '-')",
        ));
    }
    Ok(())
}
fn read(path: &Path) -> Option<Value> {
    fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
}
fn write(path: &Path, value: &Value) -> Result<()> {
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(io)?;
    writeln!(f, "{value}").map_err(io)
}
fn alive(pid: &Value) -> bool {
    pid.as_i64()
        .filter(|&p| p > 0 && p <= i32::MAX as i64)
        .is_some_and(|p| unsafe { libc::kill(p as i32, 0) == 0 })
}
fn cleanup(runs: &Path) {
    let Ok(entries) = fs::read_dir(runs) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let path = entry.path();
        let exit = read(&path.join("exit.json"));
        let run = read(&path.join("run.json"));
        if let (Some(exit), Some(run)) = (exit, run)
            && exit["t"]
                .as_f64()
                .is_some_and(|t| now() - t > 7.0 * 86400.0)
            && !alive(&run["pid"])
        {
            if let Some(tmp) = run["tmp"].as_str() {
                // Keep the record for a later retry if its temporary directory cannot be removed.
                if let Err(e) = fs::remove_dir_all(tmp)
                    && e.kind() != std::io::ErrorKind::NotFound
                {
                    continue;
                }
            }
            let _ = fs::remove_dir_all(path);
        }
    }
}

pub fn run(args: &[String]) -> Result<Value> {
    let sep = args
        .iter()
        .position(|s| s == "--")
        .ok_or_else(|| error("usage", "run requires -- command"))?;
    let mut read_paths = Vec::new();
    let mut write_paths = Vec::new();
    let mut other = Vec::new();
    let mut i = 0;
    while i < sep {
        if matches!(args[i].as_str(), "--read" | "--write") {
            let value = args
                .get(i + 1)
                .filter(|_| i + 1 < sep)
                .ok_or_else(|| error("usage", "missing path"))?;
            if args[i] == "--read" {
                read_paths.push(value.clone());
            } else {
                write_paths.push(value.clone());
            }
            i += 2;
        } else {
            other.push(args[i].clone());
            i += 1;
        }
    }
    let opts = options(&other, &["--profile", "--kind", "--agent", "--instance"])?;
    let profile = required(&opts, "--profile")?;
    let kind = required(&opts, "--kind")?;
    let agent = required(&opts, "--agent")?;
    let instance = required(&opts, "--instance")?;
    valid_instance(instance)?;
    let argv = &args[sep + 1..];
    let program = runtime::program(
        argv.first()
            .ok_or_else(|| error("usage", "missing command"))?,
    )?;
    read_paths.push(program.to_string_lossy().into_owned());
    read_paths.push(
        program
            .canonicalize()
            .map_err(io)?
            .to_string_lossy()
            .into_owned(),
    );
    let home = home()?;
    let dir = home.join("runs").join(instance);
    let cache = env::var_os("XDG_CACHE_HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".cache"));
    if !cache.is_absolute() {
        return Err(error("invalid_data", "cache directory must be absolute"));
    }
    let tmp = rules::normalize(&cache.join("fence/tmp").join(instance));
    let cwd = env::current_dir().map_err(io)?;
    let resolved = rules::resolve(
        &home,
        profile,
        Some(kind),
        &cwd,
        Some(&tmp),
        &read_paths,
        &write_paths,
    )?;
    let runtime = runtime::check()?;
    let mut command = if let Some(runner) = runtime["runner"].as_str() {
        Command::new(runner)
    } else {
        let mut c = Command::new(runtime["node"]["path"].as_str().unwrap());
        c.arg(runtime::root()?.join("share/fence/runner.mjs"));
        c
    };
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(home.join("runs"))
        .map_err(io)?;
    cleanup(&home.join("runs"));
    fs::DirBuilder::new().mode(0o700).create(&dir).map_err(io)?;
    let mut tmp_created = false;
    let prepared = (|| -> Result<()> {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(tmp.parent().unwrap())
            .map_err(io)?;
        fs::DirBuilder::new().mode(0o700).create(&tmp).map_err(io)?;
        tmp_created = true;
        for (path, contents) in &resolved.files {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(path.parent().unwrap())
                .map_err(io)?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)
                .map_err(io)?;
            file.write_all(contents.as_bytes()).map_err(io)?;
        }
        for path in &resolved.mkdir {
            fs::create_dir_all(path).map_err(io)?;
        }
        write(&dir.join("settings.json"), &resolved.value["settings"])?;
        write(
            &dir.join("run.json"),
            &json!({"agent":agent,"instance":instance,"profile":profile,"kind":kind,"cwd":resolved.value["cwd"],"tmp":tmp,"started":now(),"pid":std::process::id(),"layers":resolved.value["layers"],"monitors":{"network":false,"files":false}}),
        )?;
        Ok(())
    })();
    if let Err(mut error) = prepared {
        let mut failures = Vec::new();
        for path in [tmp_created.then_some(&tmp), Some(&dir)]
            .into_iter()
            .flatten()
        {
            if let Err(e) = fs::remove_dir_all(path) {
                failures.push(format!("{}: {e}", path.display()));
            }
        }
        if !failures.is_empty() {
            error["cleanup_errors"] = json!(failures);
        }
        return Err(error);
    }
    command
        .arg("--settings")
        .arg(dir.join("settings.json"))
        .arg("--denials")
        .arg(dir.join("denials.jsonl"))
        .arg("--exit")
        .arg(dir.join("exit.json"))
        .arg("--")
        .arg(program);
    for arg in resolved.value["args"].as_array().unwrap() {
        command.arg(arg.as_str().unwrap());
    }
    command.args(&argv[1..]);
    for (key, value) in resolved.value["env"].as_object().unwrap() {
        command.env(key, value.as_str().unwrap());
    }
    command.envs(&resolved.secret_env);
    command.env("FENCE_RUN", instance);
    let e = command.exec();
    let _ = write(
        &dir.join("exit.json"),
        &json!({"code":125,"signal":null,"t":now(),"message":e.to_string()}),
    );
    Err(error("runtime_missing", e))
}

pub fn denials(args: &[String]) -> Result<Value> {
    let opts = options(args, &["--agent", "--instance", "--limit"])?;
    let agent = required(&opts, "--agent")?;
    let instance = opts.get("--instance");
    if let Some(instance) = instance {
        valid_instance(instance)?;
    }
    let limit = opts
        .get("--limit")
        .map(|s| s.parse::<usize>())
        .transpose()
        .map_err(|_| error("usage", "invalid limit"))?
        .unwrap_or(200);
    let runs = home()?.join("runs");
    let mut candidates = Vec::new();
    if let Some(instance) = instance {
        candidates.push(runs.join(instance));
    } else if let Ok(entries) = fs::read_dir(&runs) {
        candidates.extend(
            entries
                .flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .map(|e| e.path()),
        );
    }
    let (dir, mut record) = candidates
        .into_iter()
        .filter_map(|p| Some((p.clone(), read(&p.join("run.json"))?)))
        .filter(|(_, r)| r["agent"] == agent)
        .max_by(|(_, a), (_, b)| {
            a["started"]
                .as_f64()
                .unwrap_or(0.0)
                .total_cmp(&b["started"].as_f64().unwrap_or(0.0))
        })
        .ok_or_else(|| error("not_found", "no run for this agent and instance"))?;
    let exit = read(&dir.join("exit.json"));
    record["ok"] = json!(true);
    record["running"] = json!(exit.is_none() && alive(&record["pid"]));
    record["exit"] = exit.unwrap_or(Value::Null);
    if let Some(monitor) = read(&dir.join("monitor.json")) {
        record["monitors"] = monitor["monitors"].clone();
        record["truncated"] = monitor["truncated"].clone();
    } else {
        record["truncated"] = json!(false);
    }
    let text = match fs::read_to_string(dir.join("denials.jsonl")) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(io(e)),
    };
    // The writer appends whole lines; ignore a still-in-flight final record.
    let entries: Vec<Value> = text
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let mut counts = json!({"network":0,"file-read":0,"file-write":0,"other":0});
    for entry in &entries {
        if let Some(kind) = entry["type"].as_str() {
            counts[kind] = json!(counts[kind].as_u64().unwrap_or(0) + 1);
        }
    }
    record["counts"] = counts;
    record["denials"] = json!(&entries[entries.len().saturating_sub(limit)..]);
    for key in ["pid", "layers", "cwd", "tmp"] {
        record.as_object_mut().unwrap().remove(key);
    }
    Ok(record)
}
