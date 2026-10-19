//! A running agent's policy for subsequent child starts; independent of pen versions.
use crate::{Error, Result, sandbox, state};
use serde_json::{Value, json};
use std::{fs, io::ErrorKind, path::Path};

pub fn inherited() -> Result<Option<String>> {
    let (Ok(name), Ok(instance)) = (
        std::env::var("CORRAL_NAME"),
        std::env::var("CORRAL_INSTANCE"),
    ) else {
        return Ok(None);
    };
    let Ok(dir) = state::dir(&name) else {
        return Ok(None);
    };
    // Snapshot the policy before checking liveness. Missing/cleared/stale policies
    // need no parent status; a damaged policy is ignored only for a proven old run.
    let policy = read(&dir, &json!(instance));
    if matches!(&policy, Ok(Value::Null)) || !current_instance(&dir, &instance)? {
        return Ok(None);
    }
    Ok(policy?["profile"].as_str().map(str::to_owned))
}

fn current_instance(dir: &Path, instance: &str) -> Result<bool> {
    let lock = dir.join("lock");
    if state::lock(&lock, false)
        .map_err(|e| unreadable(format!("cannot check caller lock: {}", e.value["message"])))?
        .is_some()
        || !lock.try_exists().map_err(unreadable)?
    {
        return Ok(false);
    }
    // The pen holds the run lock even when it cannot answer socket requests.
    // Missing or invalid metadata while it is held cannot prove the caller exited.
    let meta: Value = serde_json::from_slice(&fs::read(dir.join("meta.json")).map_err(unreadable)?)
        .map_err(unreadable)?;
    let current = meta["instance"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| unreadable("cannot confirm caller instance"))?;
    Ok(current == instance)
}

fn unreadable(message: impl ToString) -> Error {
    Error::new(1, "child_sandbox_unreadable", message)
}

pub fn public(dir: &Path, instance: &Value) -> Value {
    read(dir, instance).unwrap_or(Value::Null)
}

fn read(dir: &Path, instance: &Value) -> Result<Value> {
    let bytes = match fs::read(dir.join("child-sandbox.json")) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Value::Null),
        Err(e) => return Err(unreadable(e.to_string())),
    };
    let record: Value = serde_json::from_slice(&bytes).map_err(|e| unreadable(e.to_string()))?;
    let recorded_instance = record["instance"]
        .as_str()
        .ok_or_else(|| unreadable("missing or invalid instance"))?;
    if Some(recorded_instance) != instance.as_str() {
        return Ok(Value::Null);
    }
    match record.get("child_sandbox") {
        Some(Value::Null) => Ok(Value::Null),
        Some(value) if value["profile"].is_string() => Ok(json!({"profile":value["profile"]})),
        _ => Err(unreadable("missing or invalid child_sandbox")),
    }
}

pub fn run(name: &str, profile: Option<&str>, clear: bool) -> Result<Value> {
    let st = state::require(name, json!({"op":"status"}))?;
    let dir = state::dir(name)?;
    let child_sandbox = if profile.is_some() || clear {
        let value = if let Some(profile) = profile {
            sandbox::check_profile(profile)?;
            json!({"profile":profile})
        } else {
            Value::Null
        };
        if !current_instance(&dir, st["instance"].as_str().unwrap_or(""))? {
            return Err(Error::new(
                1,
                "restarted",
                "agent stopped or restarted before saving child sandbox",
            ));
        }
        state::write(
            &dir.join("child-sandbox.json"),
            &json!({"instance":st["instance"],"child_sandbox":value}),
        )?;
        value
    } else {
        public(&dir, &st["instance"])
    };
    Ok(json!({"ok":true,"name":name,"instance":st["instance"],"child_sandbox":child_sandbox}))
}
