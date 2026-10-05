// From Saddle `crates/corral-core/src/events.rs` at commit `a31dea2`, unchanged.
use crate::{Error, Result, state};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
mod raw;
use std::{
    fs::File,
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::Path,
};
pub fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim_matches(|c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c))
        .to_owned()
}
pub fn digest(s: &str) -> String {
    format!("{:x}", Sha256::digest(normalize(s).as_bytes()))
}
fn fresh(instance: &str) -> Value {
    json!({"cursor":4,"fmt":1,"inst":instance,"offset":0,"main_session":null,"other_sessions":[],"pending":[],"state":"starting","state_started":null,"last_tool":null,"turn_started":null,"last_event":null,"last_event_t":null,"inputs":[],"input_count":0,"last_prompt":null,"reply":null,"reply_t":null,"background_running":null,"raw":{}})
}
fn string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Null => "None".into(),
        _ => v.to_string(),
    }
}
fn update(s: &mut Value, e: &Value) {
    let previous = s["state"].clone();
    let ev = e["ev"].as_str().unwrap_or("");
    s["last_event"] = e["ev"].clone();
    s["last_event_t"] = e["t"].clone();
    match ev {
        "SessionStart" => {
            if e["source"] != "compact" {
                s["state"] = json!("idle")
            }
        }
        "UserPromptSubmit" => {
            s["state"] = json!("working");
            s["turn_started"] = e["t"].clone();
            s["last_tool"] = Value::Null;
            let prompt = e.get("prompt").map(string).unwrap_or_default();
            let a = s["inputs"].as_array_mut().unwrap();
            a.push(json!({"t":e["t"],"digest":digest(&prompt)}));
            if a.len() > 10 {
                a.remove(0);
            }
            s["input_count"] = json!(s["input_count"].as_u64().unwrap_or(0) + 1);
            s["last_prompt"] = json!(prompt);
        }
        "PreToolUse" | "PostToolUse" => {
            s["state"] = json!("working");
            if let Some(v) = e.get("tool_name") {
                s["last_tool"] = v.clone();
            }
        }
        "PermissionRequest" => {
            s["state"] = json!("blocked");
            if let Some(v) = e.get("tool_name") {
                s["last_tool"] = v.clone();
            }
        }
        "Notification" => {
            if e["notification_type"] == "permission_prompt" {
                s["state"] = json!("blocked")
            } else if e["notification_type"] == "idle_prompt"
                && s["state"] == "working"
                && s["background_running"] == 0
            {
                s["state"] = json!("idle")
            }
        }
        "Stop" | "StopFailure" | "Interrupt" => {
            s["state"] = json!("idle");
            if ev == "Stop" {
                s["background_running"] = e["background_running"].clone();
                if e["last_assistant_message"].is_string() {
                    s["reply"] = e["last_assistant_message"].clone();
                    s["reply_t"] = e["t"].clone();
                }
            }
        }
        "SessionEnd" => s["state"] = json!("exiting"),
        _ => {}
    }
    if s["state"] != previous {
        s["state_started"] = e["t"].clone();
    }
}
fn apply(s: &mut Value, e: &Value, cwd: &Path) {
    let sid = &e["session_id"];
    if e["ev"] == "SessionStart" {
        let same = e["cwd"]
            .as_str()
            .and_then(|p| std::fs::canonicalize(p).ok())
            .zip(std::fs::canonicalize(cwd).ok())
            .is_some_and(|(a, b)| a == b);
        if e["has_transcript"] == true && same {
            if sid != &s["main_session"] {
                s["main_session"] = sid.clone();
                s["other_sessions"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|v| v != sid);
            }
            update(s, e);
            let mut mine: Vec<Value> = s["pending"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|e| &e["session_id"] == sid)
                .cloned()
                .collect();
            mine.sort_by(|a, b| {
                a["t"]
                    .as_f64()
                    .unwrap_or(0.0)
                    .total_cmp(&b["t"].as_f64().unwrap_or(0.0))
            });
            s["pending"]
                .as_array_mut()
                .unwrap()
                .retain(|e| &e["session_id"] != sid);
            for e in mine {
                update(s, &e)
            }
        } else {
            if sid != &s["main_session"] {
                let a = s["other_sessions"].as_array_mut().unwrap();
                if !a.contains(sid) {
                    a.push(sid.clone());
                    if a.len() > 50 {
                        a.remove(0);
                    }
                }
            }
            s["pending"]
                .as_array_mut()
                .unwrap()
                .retain(|e| &e["session_id"] != sid);
        }
        return;
    }
    if sid.is_null() || sid == &s["main_session"] {
        if !s["main_session"].is_null() {
            update(s, e)
        }
    } else if !s["other_sessions"].as_array().unwrap().contains(sid) {
        let a = s["pending"].as_array_mut().unwrap();
        a.push(e.clone());
        if a.len() > 50 {
            a.remove(0);
        }
    }
}
pub fn read(dir: &Path, instance: &str, cwd: &Path) -> Result<Value> {
    let f = match File::open(dir.join("events")) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(fresh(instance)),
        Err(e) => return Err(e.into()),
    };
    let size = f.metadata()?.len();
    let mut s = state::read(dir.join("cursor"));
    if s["cursor"] != 4
        || s["inst"] != instance
        || s["offset"].as_u64().is_none_or(|n| n > size)
        || ["pending", "other_sessions", "inputs"]
            .iter()
            .any(|k| !s[*k].is_array())
    {
        s = fresh(instance)
    }
    let fmt = s.get("fmt").cloned().unwrap_or(json!(1));
    if fmt != 1 {
        return Err(Error::new(
            9,
            "incompatible",
            "unsupported event format in cursor",
        ));
    }
    s["fmt"] = fmt;
    let original = s.clone();
    let start = s["offset"].as_u64().unwrap();
    let mut input = BufReader::new(f);
    input.seek(SeekFrom::Start(start))?;
    let mut offset = start;
    loop {
        let mut line = Vec::new();
        if input.read_until(b'\n', &mut line)? == 0 || line.last() != Some(&b'\n') {
            break;
        }
        if let Ok(e) = serde_json::from_slice::<Value>(&line)
            && e.is_object()
            && e["inst"] == instance
        {
            if e["v"] != 1 && e["v"] != 2 {
                return Err(Error::new(9, "incompatible", "unsupported event format"));
            }
            raw::settle(&mut s, e["t"].as_f64().unwrap_or(0.0), cwd);
            if e["v"] == 2 {
                raw::apply_raw(&mut s, &e, cwd)?;
            } else {
                apply(&mut s, &e, cwd);
            }
        }
        offset += line.len() as u64;
    }
    s["offset"] = json!(offset);
    raw::settle(&mut s, crate::now(), cwd);
    if s != original {
        state::write(&dir.join("cursor"), &s)?
    }
    Ok(s)
}
