use crate::{Result, error};
use serde_json::{Value, json};
use std::{
    env, fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

pub fn program(name: &str) -> Result<PathBuf> {
    let path = Path::new(name);
    let candidates = if name.contains('/') {
        vec![std::path::absolute(path).map_err(|e| error("program_not_found", e))?]
    } else {
        env::split_paths(&env::var_os("PATH").unwrap_or_default())
            .map(|p| p.join(name))
            .collect()
    };
    candidates
        .into_iter()
        .find(|p| {
            p.is_file()
                && p.metadata()
                    .is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
        })
        .and_then(|p| std::path::absolute(p).ok())
        .ok_or_else(|| {
            error(
                "program_not_found",
                format!("program not executable: {name}"),
            )
        })
}

pub fn root() -> Result<PathBuf> {
    let exe = env::current_exe()
        .and_then(fs::canonicalize)
        .map_err(|e| error("runtime_missing", e))?;
    Ok(exe.parent().unwrap().parent().unwrap().to_path_buf())
}

pub fn node_version() -> Option<String> {
    let text = fs::read_to_string(root().ok()?.join("share/fence/runtime.toml")).ok()?;
    let data: toml::Value = toml::from_str(&text).ok()?;
    data.get("node_version")?.as_str().map(str::to_owned)
}

pub fn check() -> Result<Value> {
    if let Ok(runner) = env::var("FENCE_RUNNER") {
        program(&runner).map_err(|_| error("runtime_missing", "FENCE_RUNNER is not executable"))?;
        return Ok(json!({"runner":runner,"synthetic":true}));
    }
    let root = root()?;
    let runtime = root.join("share/fence/runtime.toml");
    let text = fs::read_to_string(&runtime).map_err(|e| {
        error(
            "runtime_missing",
            format!("{}: {e}; repackage fence", runtime.display()),
        )
    })?;
    let data: toml::Value = toml::from_str(&text).map_err(|e| error("runtime_missing", e))?;
    let node = data
        .get("node")
        .and_then(toml::Value::as_str)
        .filter(|s| Path::new(s).is_absolute())
        .ok_or_else(|| {
            error(
                "runtime_missing",
                "missing absolute node path; repackage fence",
            )
        })?;
    program(node).map_err(|_| {
        error(
            "runtime_missing",
            "recorded node is missing; repackage fence",
        )
    })?;
    let out = Command::new(node)
        .arg("--version")
        .output()
        .map_err(|e| error("runtime_missing", e))?;
    let version = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    let mut numbers = version
        .trim_start_matches('v')
        .split('.')
        .filter_map(|p| p.parse::<u32>().ok());
    if !out.status.success()
        || (numbers.next().unwrap_or(0), numbers.next().unwrap_or(0)) < (22, 12)
    {
        return Err(error(
            "runtime_missing",
            "node >= 22.12 is required; repackage fence",
        ));
    }
    let srt = root.join("lib/srt/node_modules/@anthropic-ai/sandbox-runtime/package.json");
    let package: Value = fs::read(&srt)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null);
    if package["version"] != "0.0.80" || !root.join("share/fence/runner.mjs").is_file() {
        return Err(error(
            "runtime_missing",
            "srt 0.0.80 or runner is missing; repackage fence",
        ));
    }
    if cfg!(target_os = "linux") {
        for name in ["bwrap", "socat", "rg"] {
            program(name)
                .map_err(|_| error("runtime_missing", format!("Linux requires {name}")))?;
        }
    }
    Ok(
        json!({"node":{"path":node,"version":version,"ok":true},"srt":{"version":"0.0.80","ok":true}}),
    )
}
