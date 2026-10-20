use crate::{Result, error};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path};
include!(concat!(env!("OUT_DIR"), "/data.rs"));

pub fn names(home: &Path, group: &str) -> Result<BTreeSet<String>> {
    let mut names: BTreeSet<_> = BUILTIN
        .iter()
        .filter(|(g, _, _)| *g == group)
        .map(|(_, n, _)| n.to_string())
        .collect();
    match fs::read_dir(home.join(group)) {
        Ok(entries) => {
            for entry in entries {
                let path = entry.map_err(|e| error("invalid_data", e))?.path();
                if path.extension().is_some_and(|e| e == "toml")
                    && let Some(name) = path.file_stem().and_then(|s| s.to_str())
                {
                    names.insert(name.into());
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(error("invalid_data", e)),
    }
    Ok(names)
}

pub fn problem(source: &str, path: &str, message: impl ToString) -> Value {
    let mut e = error("invalid_data", "invalid fence data");
    e["problems"] = json!([{"source":source,"path":path,"message":message.to_string()}]);
    e
}

pub fn load(home: &Path, group: &str, name: &str) -> Result<(Value, Value)> {
    if name.is_empty() || name.contains(['/', '\\']) || name == "." || name == ".." {
        return Err(problem(group, name, "invalid data name"));
    }
    let path = home.join(group).join(format!("{name}.toml"));
    let source = match group {
        "profiles" => "profile",
        "agents" => "agent",
        "mcp" => "mcp",
        _ => "service",
    };
    let builtin = BUILTIN.iter().find(|(g, n, _)| *g == group && *n == name);
    let (text, origin) = match fs::read_to_string(&path) {
        Ok(text) => (text, "user"),
        Err(e)
            if e.kind() == std::io::ErrorKind::NotFound && fs::symlink_metadata(&path).is_err() =>
        {
            let Some((_, _, text)) = builtin else {
                let kind = match group {
                    "profiles" => "unknown_profile",
                    "agents" => "unsupported_kind",
                    _ => "invalid_data",
                };
                let mut e = error(kind, format!("no {source} named {name}"));
                if group == "agents" {
                    e["supported"] = json!(names(home, group)?);
                }
                if group == "services" {
                    return Err(problem(source, name, "unknown service"));
                }
                return Err(e);
            };
            (text.to_string(), "builtin")
        }
        Err(e) => return Err(problem(&path.to_string_lossy(), "", e)),
    };
    let label = if origin == "builtin" {
        format!("builtin:{group}/{name}.toml")
    } else {
        path.to_string_lossy().into_owned()
    };
    let value = parse(&text, &label)?;
    crate::rules::validate(&value, &label, source, "")?;
    Ok((value, json!({"source":source,"name":name,"origin":origin})))
}

pub fn parse(text: &str, source: &str) -> Result<Value> {
    let value: toml::Value = toml::from_str(text).map_err(|e| problem(source, "", e))?;
    serde_json::to_value(value).map_err(|e| problem(source, "", e))
}

pub fn profiles(home: &Path) -> Result<Value> {
    let mut output = json!({"ok":true,"local":{"path":home.join("local.toml"),"exists":home.join("local.toml").exists()}});
    for (group, field) in [
        ("profiles", "profiles"),
        ("agents", "kinds"),
        ("services", "services"),
        ("mcp", "mcp"),
    ] {
        let mut entries = Vec::new();
        for name in names(home, group)? {
            let path = home.join(group).join(format!("{name}.toml"));
            let user = fs::symlink_metadata(&path).is_ok();
            let mut item = json!({"source":if user {"user"} else {"builtin"}});
            item[if group == "agents" { "kind" } else { "name" }] = json!(name);
            if group == "profiles" {
                item["path"] = if user { json!(path) } else { Value::Null };
                item["overrides_builtin"] =
                    json!(user && BUILTIN.iter().any(|(g, n, _)| *g == group && *n == name));
            }
            match load(home, group, &name) {
                Ok((v, _)) => {
                    if group == "profiles" {
                        item["description"] = v["description"].clone();
                    }
                    if group == "agents" {
                        item["services"] = v.get("services").cloned().unwrap_or(json!([]));
                    }
                }
                Err(e) => {
                    item["error"] = e;
                }
            }
            entries.push(item);
        }
        output[field] = json!(entries);
    }
    Ok(output)
}
