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
    let Ok(st) = state::require(&name, json!({"op":"status"})) else {
        return Ok(None);
    };
    if st["instance"].as_str() != Some(&instance) {
        return Ok(None);
    }
    Ok(read(&state::dir(&name)?, &st["instance"])?["profile"]
        .as_str()
        .map(str::to_owned))
}

pub fn public(dir: &Path, instance: &Value) -> Value {
    read(dir, instance).unwrap_or(Value::Null)
}

fn read(dir: &Path, instance: &Value) -> Result<Value> {
    let unreadable = |message: String| Error::new(1, "child_sandbox_unreadable", message);
    let bytes = match fs::read(dir.join("child-sandbox.json")) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Value::Null),
        Err(e) => return Err(unreadable(e.to_string())),
    };
    let record: Value = serde_json::from_slice(&bytes).map_err(|e| unreadable(e.to_string()))?;
    let recorded_instance = record["instance"]
        .as_str()
        .ok_or_else(|| unreadable("missing or invalid instance".into()))?;
    if Some(recorded_instance) != instance.as_str() {
        return Ok(Value::Null);
    }
    match record.get("child_sandbox") {
        Some(Value::Null) => Ok(Value::Null),
        Some(value) if value["profile"].is_string() => Ok(json!({"profile":value["profile"]})),
        _ => Err(unreadable("missing or invalid child_sandbox".into())),
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
