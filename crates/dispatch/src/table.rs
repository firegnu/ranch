//! Local, data-driven dispatch choices. This module never starts an agent.
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::ErrorKind,
    path::PathBuf,
};

const BUILTIN: &str = include_str!("../resources/dispatch.toml");

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Data {
    #[serde(default)]
    agents: BTreeMap<String, toml::Table>,
    #[serde(default)]
    cells: BTreeMap<String, toml::Table>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Agent {
    argv: Vec<String>,
    #[serde(default)]
    labels: Vec<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Cell {
    agents: Vec<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    effort: Option<String>,
    #[serde(default)]
    cross_review: Option<String>,
    #[serde(default)]
    rework: Option<String>,
}

/// Reads the effective table, or one cell, using HOME without creating any files.
pub fn read(name: Option<&str>) -> Result<Value, (u8, Value)> {
    let home = std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            (
                1,
                json!({"ok": false, "error": "invalid_data", "message": "HOME is not set"}),
            )
        })?;
    let path = PathBuf::from(home).join(".ranch/dispatch.toml");
    let table = load(&path).map_err(|message| {
        (
            1,
            json!({"ok": false, "error": "invalid_data",
                   "message": format!("{}: {message}", path.display())}),
        )
    })?;
    match name {
        None => Ok(table),
        Some(name) => match table["cells"].get(name) {
            Some(cell) => Ok(json!({"ok": true, "cell": cell})),
            None => Err((
                1,
                json!({"ok": false, "error": "unknown_cell",
                                   "message": format!("unknown cell: {name}")}),
            )),
        },
    }
}

fn load(path: &std::path::Path) -> Result<Value, String> {
    let mut data: Data = toml::from_str(BUILTIN).map_err(|e| format!("builtin: {e}"))?;
    let local: Option<Data> = match fs::read_to_string(path) {
        Ok(text) => Some(toml::from_str(&text).map_err(|e| format!("{e}"))?),
        Err(e) if e.kind() == ErrorKind::NotFound => None,
        Err(e) => return Err(e.to_string()),
    };
    let mut user_agents = BTreeSet::new();
    let mut user_cells = BTreeSet::new();
    if let Some(local) = &local {
        merge(&mut data.agents, &local.agents, &mut user_agents);
        merge(&mut data.cells, &local.cells, &mut user_cells);
    }
    let agents: BTreeMap<String, Agent> = decode(data.agents, "agents")?;
    let mut cells: BTreeMap<String, Cell> = decode(data.cells, "cells")?;
    for (name, agent) in &agents {
        if agent.argv.is_empty() || agent.argv[0].trim().is_empty() {
            return Err(format!("agents.{name}.argv: needs a program"));
        }
        // Check templates even in families which no cell currently uses.
        for (field, values) in [("argv", &agent.argv), ("labels", &agent.labels)] {
            for (i, value) in values.iter().enumerate() {
                expand(value, Some("model"), Some("effort"))
                    .map_err(|e| format!("agents.{name}.{field}[{i}]: {e}"))?;
            }
        }
    }
    for (name, cell) in &mut cells {
        for field in [
            &mut cell.model,
            &mut cell.effort,
            &mut cell.cross_review,
            &mut cell.rework,
        ] {
            if field.as_deref() == Some("") {
                *field = None;
            }
        }
        if cell.agents.is_empty() {
            return Err(format!("cells.{name}.agents: needs at least one agent"));
        }
        for agent in &cell.agents {
            if !agents.contains_key(agent) {
                return Err(format!("cells.{name}.agents: unknown agent {agent:?}"));
            }
        }
    }
    for (name, cell) in &cells {
        for (field, target) in [
            ("cross_review", &cell.cross_review),
            ("rework", &cell.rework),
        ] {
            if let Some(target) = target
                && !cells.contains_key(target)
            {
                return Err(format!("cells.{name}.{field}: unknown cell {target:?}"));
            }
        }
    }
    let mut done = BTreeSet::new();
    for name in cells.keys() {
        check_cycles(name, &cells, &mut Vec::new(), &mut done)?;
    }
    let mut resolved = BTreeMap::new();
    for (name, cell) in &cells {
        let mut choices = Vec::new();
        for family in &cell.agents {
            let agent = &agents[family];
            let render = |values: &[String], field: &str| -> Result<Vec<String>, String> {
                values
                    .iter()
                    .enumerate()
                    .map(|(i, value)| {
                        expand(value, cell.model.as_deref(), cell.effort.as_deref()).map_err(|e| {
                            format!("cells.{name} / agents.{family}.{field}[{i}]: {e}")
                        })
                    })
                    .collect()
            };
            let argv = render(&agent.argv, "argv")?;
            let labels = render(&agent.labels, "labels")?;
            choices.push(json!({
                "agent": family, "source": source(&user_agents, family),
                "command": shell_words::join(&argv), "argv": argv, "labels": labels,
            }));
        }
        let mut value = serde_json::to_value(cell).expect("cell JSON");
        value["name"] = json!(name);
        value["source"] = json!(source(&user_cells, name));
        value["choices"] = json!(choices);
        resolved.insert(name.clone(), value);
    }
    // A review is a one-level launch description, not a recursively expanded table.
    let reviews: BTreeMap<_, _> = resolved
        .iter()
        .map(|(name, cell)| {
            (
                name.clone(),
                json!({"name": name, "source": cell["source"],
                            "model": cell["model"], "effort": cell["effort"],
                            "choices": cell["choices"]}),
            )
        })
        .collect();
    for (name, value) in &mut resolved {
        value["review"] = cells[name]
            .cross_review
            .as_ref()
            .map(|target| reviews[target].clone())
            .unwrap_or(Value::Null);
    }
    let families: BTreeMap<_, _> = agents
        .iter()
        .map(|(name, agent)| {
            let mut value = serde_json::to_value(agent).expect("agent JSON");
            value["source"] = json!(source(&user_agents, name));
            (name, value)
        })
        .collect();
    Ok(json!({"ok": true, "agents": families, "cells": resolved,
              "local": {"path": path, "exists": local.is_some()}}))
}

fn merge(
    base: &mut BTreeMap<String, toml::Table>,
    patch: &BTreeMap<String, toml::Table>,
    sources: &mut BTreeSet<String>,
) {
    for (name, fields) in patch {
        base.entry(name.clone()).or_default().extend(fields.clone());
        sources.insert(name.clone());
    }
}

fn decode<T: DeserializeOwned>(
    entries: BTreeMap<String, toml::Table>,
    section: &str,
) -> Result<BTreeMap<String, T>, String> {
    entries
        .into_iter()
        .map(|(name, fields)| {
            if name.trim().is_empty() {
                return Err(format!("{section}: empty name"));
            }
            let entry = toml::Value::Table(fields)
                .try_into()
                .map_err(|e| format!("{section}.{name}: {e}"))?;
            Ok((name, entry))
        })
        .collect()
}

fn source(user: &BTreeSet<String>, name: &str) -> &'static str {
    if user.contains(name) {
        "user"
    } else {
        "builtin"
    }
}

fn expand(template: &str, model: Option<&str>, effort: Option<&str>) -> Result<String, String> {
    let mut output = String::new();
    let mut rest = template;
    while let Some((before, tail)) = rest.split_once("${") {
        output.push_str(before);
        let (key, after) = tail.split_once('}').ok_or("unclosed placeholder")?;
        let value = match key {
            "model" => model,
            "effort" => effort,
            _ => return Err(format!("unknown placeholder ${{{key}}}")),
        }
        .ok_or_else(|| format!("missing {key} for ${{{key}}}"))?;
        output.push_str(value);
        rest = after;
    }
    output.push_str(rest);
    Ok(output)
}

fn check_cycles(
    name: &str,
    cells: &BTreeMap<String, Cell>,
    path: &mut Vec<String>,
    done: &mut BTreeSet<String>,
) -> Result<(), String> {
    if done.contains(name) {
        return Ok(());
    }
    if path.iter().any(|item| item == name) {
        path.push(name.to_owned());
        return Err(format!("cells: reference cycle: {}", path.join(" -> ")));
    }
    path.push(name.to_owned());
    let cell = &cells[name];
    for target in [&cell.cross_review, &cell.rework].into_iter().flatten() {
        check_cycles(target, cells, path, done)?;
    }
    path.pop();
    done.insert(name.to_owned());
    Ok(())
}
