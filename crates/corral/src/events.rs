// From Saddle `crates/corral-core/src/events.rs` at commit `a31dea2`; ranch has changed it since.
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
    json!({"cursor":8,"fmt":1,"inst":instance,"offset":0,"main_session":null,"other_sessions":[],"pending":[],"state":"starting","interrupted":false,"state_started":null,"last_tool":null,"turn_started":null,"last_event":null,"last_event_t":null,"inputs":[],"input_count":0,"last_prompt":null,"reply":null,"reply_t":null,"background_running":null,"raw":{}})
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
    if let Some(path) = e["transcript_path"].as_str().filter(|p| !p.is_empty()) {
        s["transcript_path"] = json!(path);
    }
    let previous = s["state"].clone();
    let ev = e["ev"].as_str().unwrap_or("");
    s["last_event"] = e["ev"].clone();
    s["last_event_t"] = e["t"].clone();
    match ev {
        "SessionStart" => {
            if e["source"] != "compact" {
                s["turn_open"] = json!(false);
                s["state"] = json!("idle")
            }
        }
        "UserPromptSubmit" => {
            s["turn_open"] = json!(true);
            s["state"] = json!("working");
            s["interrupted"] = json!(false);
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
            // Closing an idle UI prompt is not the end of a new model turn.
            if ev != "Stop" || e["idle_only"] != true {
                s["turn_open"] = json!(false);
                s["interrupted"] = json!(ev == "Interrupt");
            }
            if ev == "Stop" {
                s["background_running"] = e["background_running"].clone();
                if e["last_assistant_message"].is_string() {
                    s["reply"] = e["last_assistant_message"].clone();
                    s["reply_t"] = e["t"].clone();
                }
            }
        }
        "SessionEnd" => {
            s["turn_open"] = json!(false);
            s["state"] = json!("exiting");
        }
        _ => {}
    }
    if s["state"] != previous {
        s["state_started"] = e["t"].clone();
    }
}
/// Whether an event comes from the agent's own directory.
fn same_dir(e: &Value, cwd: &Path) -> bool {
    e["cwd"]
        .as_str()
        .and_then(|p| std::fs::canonicalize(p).ok())
        .zip(std::fs::canonicalize(cwd).ok())
        .is_some_and(|(a, b)| a == b)
}
/// Makes `sid` the main session and takes its pending events, oldest first.
fn make_main(s: &mut Value, sid: &Value) -> Vec<Value> {
    if sid != &s["main_session"] {
        s.as_object_mut().unwrap().remove("transcript_path");
        s["turn_open"] = json!(false);
        s["main_session"] = sid.clone();
        s["other_sessions"]
            .as_array_mut()
            .unwrap()
            .retain(|v| v != sid);
    }
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
    mine
}
fn apply(s: &mut Value, e: &Value, cwd: &Path) {
    let sid = &e["session_id"];
    if e["ev"] == "SessionStart" {
        if e["has_transcript"] == true && same_dir(e, cwd) {
            let mine = make_main(s, sid);
            update(s, e);
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
/// For an agent that can lose its session start (`Adapter::adopts_session`): with no main
/// session yet, the session of the oldest pending event from its directory becomes the main one.
fn adopt(s: &mut Value, cwd: &Path) {
    if !s["main_session"].is_null() {
        return;
    }
    let Some(sid) = s["pending"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["has_transcript"] == true && !e["session_id"].is_null() && same_dir(e, cwd))
        .min_by(|a, b| {
            a["t"]
                .as_f64()
                .unwrap_or(0.0)
                .total_cmp(&b["t"].as_f64().unwrap_or(0.0))
        })
        .map(|e| e["session_id"].clone())
    else {
        return;
    };
    for e in make_main(s, &sid) {
        update(s, &e)
    }
}
/// The agent's state from its events; `adopt_session` is the agent's `Adapter::adopts_session`.
pub fn read(dir: &Path, instance: &str, cwd: &Path, adopt_session: bool) -> Result<Value> {
    let f = match File::open(dir.join("events")) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(fresh(instance)),
        Err(e) => return Err(e.into()),
    };
    let size = f.metadata()?.len();
    let mut s = state::read(dir.join("cursor"));
    // Older readers do not recognise omp user interruptions (or retain Claude's turn evidence).
    // Replay their caches; the event format is unchanged in both directions.
    if s["cursor"] != 8
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
    if adopt_session {
        adopt(&mut s, cwd);
    }
    raw::settle(&mut s, crate::now(), cwd);
    if s != original {
        state::write(&dir.join("cursor"), &s)?
    }
    Ok(s)
}

/// Recover interruption evidence for an unfinished turn, including one made idle by a
/// notification. A normal end closes the turn; new input and session selection bind its evidence.
/// Late tool hooks can make an interrupted turn working again; recheck its evidence then,
/// while a stable interrupted idle needs no further reads.
pub(crate) fn observe_transcript(
    dir: &Path,
    s: &mut Value,
    adapter: &dyn crate::agents::Adapter,
) -> Result<()> {
    let needs_transcript = match s["state"].as_str() {
        Some("working") => s["turn_open"] == true || s["interrupted"] == true,
        Some("idle") => s["turn_open"] == true,
        _ => false,
    };
    if needs_transcript && let Some(t) = adapter.transcript_interrupt(s) {
        update(s, &json!({"ev":"Interrupt", "t":t}));
        state::write(&dir.join("cursor"), s)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn state(events: &[Value], adopt: bool) -> Value {
        let dir = tempfile::tempdir().unwrap();
        let mut f = File::create(dir.path().join("events")).unwrap();
        for e in events {
            let mut e = e.clone();
            e["v"] = json!(1);
            e["inst"] = json!("i");
            e["has_transcript"] = json!(true);
            if e.get("cwd").is_none() {
                e["cwd"] = json!(dir.path());
            }
            writeln!(f, "{e}").unwrap();
        }
        read(dir.path(), "i", dir.path(), adopt).unwrap()
    }

    #[test]
    fn a_session_that_lost_its_start_is_adopted_only_where_the_adapter_says_so() {
        let turn = [
            json!({"ev":"UserPromptSubmit","session_id":"a","t":2.0,"prompt":"go"}),
            json!({"ev":"PostToolUse","session_id":"a","t":1.0,"tool_name":"ls"}),
            json!({"ev":"Stop","session_id":"a","t":3.0,"last_assistant_message":"DONE"}),
        ];
        // Other agents keep waiting for the session start.
        let s = state(&turn, false);
        assert_eq!(
            (&s["state"], &s["main_session"]),
            (&json!("starting"), &Value::Null)
        );
        assert_eq!(s["pending"].as_array().unwrap().len(), 3);
        // agy's session is adopted and its events count in time order.
        let s = state(&turn, true);
        assert_eq!(s["main_session"], "a");
        assert_eq!((&s["state"], &s["reply"]), (&json!("idle"), &json!("DONE")));
        assert_eq!(
            (&s["input_count"], &s["last_tool"]),
            (&json!(1), &Value::Null)
        );
        assert!(s["pending"].as_array().unwrap().is_empty());
        // Only once it reports: before that it still starts.
        assert_eq!(state(&[], true)["state"], "starting");
    }

    #[test]
    fn adoption_takes_the_oldest_session_from_the_agents_directory_and_yields_to_a_later_start() {
        let elsewhere = json!({"ev":"PostToolUse","session_id":"x","t":1.0,"cwd":"/"});
        let mine = json!({"ev":"UserPromptSubmit","session_id":"a","t":2.0,"prompt":"go"});
        let later = json!({"ev":"UserPromptSubmit","session_id":"b","t":3.0,"prompt":"go"});
        let s = state(&[elsewhere, mine.clone(), later], true);
        assert_eq!(
            (&s["main_session"], &s["state"]),
            (&json!("a"), &json!("working"))
        );
        // A session start that comes afterwards switches sessions as before.
        let start = json!({"ev":"SessionStart","session_id":"c","t":4.0,"source":"startup"});
        let s = state(&[mine, start], true);
        assert_eq!(
            (&s["main_session"], &s["state"]),
            (&json!("c"), &json!("idle"))
        );
    }
}
