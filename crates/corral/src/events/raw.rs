// From Saddle `crates/corral-core/src/events/raw.rs` at commit `a31dea2`, unchanged.
//! Version 2 collectors report facts. All turn judgement lives in this reader.
use super::*;
use regex::Regex;
use std::sync::LazyLock;
static RETRYABLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)overloaded|rate.?limit|too many requests|\b(429|500|502|503|504)\b|unavailable|server.?error|internal.?error|network|connection|socket|fetch failed|timed? ?out|timeout|terminated").unwrap()
});

pub(super) fn settle(s: &mut Value, at: f64, cwd: &Path) {
    let mut ready = Vec::new();
    if let Some(sessions) = s["raw"].as_object_mut() {
        for session in sessions.values_mut() {
            if session["stop"]["t"].as_f64().is_some_and(|t| t <= at) {
                ready.push(session["stop"].take());
            }
        }
    }
    ready.sort_by(|a, b| {
        a["t"]
            .as_f64()
            .unwrap()
            .total_cmp(&b["t"].as_f64().unwrap())
    });
    for e in ready {
        apply(s, &e, cwd);
    }
}

pub(super) fn apply_raw(s: &mut Value, e: &Value, cwd: &Path) -> Result<()> {
    let adapter = e["adapter"].as_str().unwrap_or("");
    if !matches!(adapter, "pi" | "omp") {
        return Err(Error::new(
            9,
            "incompatible",
            "unsupported raw event adapter",
        ));
    }
    if adapter == "omp" && e["has_ui"] != true {
        return Ok(());
    }
    let key = e["session_id"].as_str().unwrap_or("");
    if !s["raw"].is_object() {
        s["raw"] = json!({});
    }
    if !s["raw"][key].is_object() {
        s["raw"][key] = json!({"inputs":[],"reply":null,"stop":null});
    }
    let r = &mut s["raw"][key];
    let body = &e["event"];
    let mut mapped = json!({"v":1,"inst":e["inst"],"t":e["t"],"session_id":e["session_id"],"cwd":e["cwd"],"has_transcript":true});
    let kind = match e["ev"].as_str().unwrap_or("") {
        "session_start" | "session_switch" => {
            r["stop"] = Value::Null;
            mapped["source"] = body["reason"].clone();
            "SessionStart"
        }
        "input" => {
            if let Some(text) = body["text"].as_str() {
                if body["streamingBehavior"].is_null() || body["streamingBehavior"] == false {
                    r["inputs"] = json!([]);
                }
                r["inputs"].as_array_mut().unwrap().push(json!(text));
            }
            return Ok(());
        }
        "before_agent_start" => {
            r["reply"] = Value::Null;
            r["stop"] = Value::Null;
            let inputs = r["inputs"].as_array_mut().unwrap();
            mapped["prompt"] = if inputs.is_empty() {
                body["prompt"].clone()
            } else {
                inputs.remove(0)
            };
            "UserPromptSubmit"
        }
        "agent_start" => {
            r["stop"] = Value::Null;
            return Ok(());
        }
        "tool_execution_start" if adapter == "omp" && body["toolName"] == "ask" => {
            mapped["notification_type"] = json!("permission_prompt");
            "Notification"
        }
        "tool_execution_start" => {
            mapped["tool_name"] = body["toolName"].clone();
            "PreToolUse"
        }
        "tool_execution_end" | "tool_approval_resolved" => {
            mapped["tool_name"] = body["toolName"].clone();
            "PostToolUse"
        }
        "tool_approval_requested" | "ui_prompt_start" => {
            mapped["notification_type"] = json!("permission_prompt");
            "Notification"
        }
        "ui_prompt_end" => {
            if e["idle"] == true {
                "Stop"
            } else {
                "PostToolUse"
            }
        }
        "message_end" => {
            let m = &body["message"];
            if m["role"] == "assistant"
                && let Some(parts) = m["content"].as_array()
            {
                let text = parts
                    .iter()
                    .filter(|p| p["type"] == "text")
                    .filter_map(|p| p["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                if !text.is_empty() {
                    r["reply"] = json!(text);
                }
            }
            return Ok(());
        }
        "agent_settled" => {
            mapped["last_assistant_message"] = r["reply"].clone();
            "Stop"
        }
        "agent_end" => {
            if body["willContinue"] != true {
                let retry = body["messages"]
                    .as_array()
                    .and_then(|a| a.iter().rev().find(|m| m["role"] == "assistant"))
                    .is_some_and(|m| {
                        m["stopReason"] == "error"
                            && RETRYABLE.is_match(m["errorMessage"].as_str().unwrap_or(""))
                    });
                mapped["ev"] = json!("Stop");
                mapped["last_assistant_message"] = r["reply"].clone();
                mapped["t"] =
                    json!(e["t"].as_f64().unwrap_or(0.0) + if retry { 2.5 } else { 0.25 });
                r["stop"] = mapped;
            }
            return Ok(());
        }
        "session_shutdown" => {
            r["stop"] = Value::Null;
            if adapter == "pi" && !body["reason"].is_null() && body["reason"] != "quit" {
                return Ok(());
            }
            "SessionEnd"
        }
        _ => return Ok(()),
    };
    mapped["ev"] = json!(kind);
    apply(s, &mapped, cwd);
    Ok(())
}
