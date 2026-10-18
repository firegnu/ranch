//! The fence command protocol. No fence data or state layout is known here.
use crate::{Error, Result, agents, state};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub fn check(profile: &str, program: &str, cwd: &Path, name: &str, extra: &Value) -> Result<Value> {
    let fence = std::env::var_os("CORRAL_FENCE")
        .map(PathBuf::from)
        .unwrap_or(crate::executable()?.with_file_name("fence"));
    if !fence.is_absolute() {
        return Err(Error::new(
            1,
            "sandbox_unavailable",
            "CORRAL_FENCE must be absolute",
        ));
    }
    let (environment, _) =
        crate::environment::build(name, "", &state::home().join("events"), extra);
    let out = Command::new(&fence)
        .args([
            "check",
            "--profile",
            profile,
            "--kind",
            &agents::kind(program),
            "--cwd",
        ])
        .arg(cwd)
        .env_clear()
        .envs(environment)
        .output()
        .map_err(|e| Error::new(1, "sandbox_unavailable", e))?;
    let value: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(
        |_| json!({"ok":false,"error":"bad_reply","message":"fence check did not return JSON"}),
    );
    if !out.status.success() || value["ok"] != true {
        return Err(Error::new(1, "sandbox_failed", "fence check failed").with("fence", value));
    }
    Ok(json!({"profile":profile,"fence":fence}))
}

pub fn command(
    sandbox: &Value,
    kind: &str,
    name: &str,
    instance: &str,
    dir: &Path,
    argv: &[String],
) -> Result<Command> {
    let mut command = Command::new(sandbox["fence"].as_str().unwrap());
    command.args([
        "run",
        "--profile",
        sandbox["profile"].as_str().unwrap(),
        "--kind",
        kind,
        "--agent",
        name,
        "--instance",
        instance,
    ]);
    let exe = crate::executable()?;
    let versions = exe
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or_else(|| {
            Error::new(
                1,
                "sandbox_unavailable",
                "cannot locate runtime version parent",
            )
        })?;
    for path in [
        state::home().join(".runtime"),
        versions.to_path_buf(),
        dir.to_path_buf(),
    ] {
        command.arg("--read").arg(std::path::absolute(path)?);
    }
    command
        .arg("--write")
        .arg(dir.join("events"))
        .arg("--")
        .args(argv);
    Ok(command)
}

pub fn public(meta: &Value) -> Value {
    meta["sandbox"]["profile"]
        .as_str()
        .map(|profile| json!({"profile":profile}))
        .unwrap_or(Value::Null)
}
