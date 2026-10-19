// From Saddle `crates/corral-core/src/events/raw.rs` at commit `a31dea2`; ranch has changed it since.
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
    let Some(adapter) =
        crate::agents::find(e["adapter"].as_str().unwrap_or("")).filter(|a| a.raw_events())
    else {
        return Err(Error::new(
            9,
            "incompatible",
            "unsupported raw event adapter",
        ));
    };
    if adapter.raw_ignored(e) {
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
        "tool_execution_start" if adapter.raw_asks(&body["toolName"]) => {
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
                mapped["idle_only"] = json!(true);
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
            if body["aborted"] == true {
                "Interrupt"
            } else {
                "Stop"
            }
        }
        "agent_end" => {
            if body["willContinue"] != true {
                let last = body["messages"]
                    .as_array()
                    .and_then(|a| a.iter().rev().find(|m| m["role"] == "assistant"));
                let retry = last.is_some_and(|m| {
                    m["stopReason"] == "error"
                        && RETRYABLE.is_match(m["errorMessage"].as_str().unwrap_or(""))
                });
                let interrupted = adapter.kind() == "omp"
                    && last.is_some_and(|m| {
                        m["stopReason"] == "aborted"
                            && m["errorMessage"] == "Interrupted by user"
                            // omp's UserInterrupt flag; errorId also carries other bits.
                            && m["errorId"].as_u64().is_some_and(|id| id & 67108864 != 0)
                    });
                mapped["ev"] = json!(if interrupted { "Interrupt" } else { "Stop" });
                mapped["last_assistant_message"] = r["reply"].clone();
                mapped["t"] =
                    json!(e["t"].as_f64().unwrap_or(0.0) + if retry { 2.5 } else { 0.25 });
                r["stop"] = mapped;
            }
            return Ok(());
        }
        "session_shutdown" => {
            r["stop"] = Value::Null;
            if !adapter.raw_shutdown_ends(&body["reason"]) {
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

#[cfg(test)]
mod tests {
    use super::*;

    struct Feed {
        adapter: &'static str,
        t: f64,
        events: Vec<Value>,
    }
    impl Feed {
        fn new(adapter: &'static str) -> Self {
            Self {
                adapter,
                t: 100.0,
                events: vec![],
            }
        }
        fn at(&mut self, dt: f64, ev: &str, body: Value) -> &mut Self {
            self.t += dt;
            self.events.push(json!({"v":2,"inst":"inst","adapter":self.adapter,"ev":ev,"t":self.t,"session_id":"s1","cwd":"/tmp","has_ui":true,"idle":false,"event":body}));
            self
        }
        fn ev(&mut self, ev: &str, body: Value) -> &mut Self {
            self.at(1.0, ev, body)
        }
        fn with(&mut self, key: &str, value: Value) -> &mut Self {
            self.events.last_mut().unwrap()[key] = value;
            self
        }
        /// Replays the events the way `events::read` does and records the state after each one.
        fn run(&self) -> Value {
            let cwd = Path::new("/tmp");
            let mut s = super::super::fresh("inst");
            let mut steps = vec![];
            for e in &self.events {
                settle(&mut s, e["t"].as_f64().unwrap_or(0.0), cwd);
                let error = apply_raw(&mut s, e, cwd).err().map(|e| e.value);
                steps.push(json!({"event":e,"error":error,"state":s.clone()}));
            }
            settle(&mut s, f64::MAX, cwd);
            json!({"steps":steps,"settled":s})
        }
    }
    fn assistant(texts: &[&str]) -> Value {
        let mut content: Vec<Value> = texts
            .iter()
            .map(|t| json!({"type":"text","text":t}))
            .collect();
        content.push(json!({"type":"thinking","thinking":"hidden"}));
        json!({"message":{"role":"assistant","content":content}})
    }

    #[test]
    fn omp_user_interrupt_is_not_normal_completion() {
        let message = json!({"role":"assistant","content":[],"stopReason":"aborted","errorMessage":"Interrupted by user","errorId":67112960});
        let result = Feed::new("omp")
            .ev("session_start", json!({"reason":"startup"}))
            .ev("before_agent_start", json!({"prompt":"first"}))
            .ev("message_end", assistant(&["previous reply"]))
            .ev("agent_end", json!({}))
            .ev("before_agent_start", json!({"prompt":"second"}))
            .ev("message_end", json!({"message":message}))
            .ev("agent_end", json!({"messages":[message]}))
            .run();
        assert_eq!(result["settled"]["state"], "idle");
        assert_eq!(result["settled"]["last_event"], "Interrupt");
        assert_eq!(result["settled"]["interrupted"], true);
    }

    #[test]
    fn omp_requires_all_user_interrupt_markers_on_the_last_assistant() {
        for message in [
            json!({"role":"assistant","stopReason":"stop"}),
            json!({"role":"assistant","stopReason":"aborted","errorMessage":"Request was aborted","errorId":4096}),
            json!({"role":"assistant","stopReason":"aborted","errorMessage":"Interrupted by user","errorId":4096}),
            json!({"role":"assistant","stopReason":"aborted","errorMessage":"Interrupted by user"}),
            json!({"role":"assistant","stopReason":"aborted","errorMessage":"Request was aborted","errorId":67112960}),
            json!({"role":"assistant","stopReason":"error","errorMessage":"Interrupted by user","errorId":67112960}),
        ] {
            let result = Feed::new("omp")
                .ev("session_start", json!({"reason":"startup"}))
                .ev("before_agent_start", json!({"prompt":"work"}))
                .ev("agent_end", json!({"messages":[
                    {"role":"assistant","stopReason":"aborted","errorMessage":"Interrupted by user","errorId":67112960},
                    message,
                    {"role":"toolResult","isError":true,"content":[{"type":"text","text":"[Command aborted]"}]}
                ]}))
                .run();
            assert_eq!(result["settled"]["last_event"], "Stop", "{message}");
            assert_eq!(result["settled"]["interrupted"], false, "{message}");
        }
    }

    #[test]
    fn omp_continuation_tool_cancel_and_retry_keep_existing_behavior() {
        let interrupted = json!({"role":"assistant","stopReason":"aborted","errorMessage":"Interrupted by user","errorId":67112960});
        let result = Feed::new("omp")
            .ev("session_start", json!({"reason":"startup"}))
            .ev("before_agent_start", json!({"prompt":"work"}))
            .ev("message_end", json!({"message":interrupted}))
            .ev(
                "agent_end",
                json!({"willContinue":true,"messages":[interrupted]}),
            )
            .run();
        assert_eq!(result["settled"]["state"], "working");
        assert_eq!(result["settled"]["interrupted"], false);

        let result = Feed::new("omp")
            .ev("session_start", json!({"reason":"startup"}))
            .ev("before_agent_start", json!({"prompt":"work"}))
            .ev("tool_execution_start", json!({"toolName":"bash"}))
            .ev("tool_execution_end", json!({"toolName":"bash","isError":true}))
            .ev("message_end", json!({"message":{"role":"toolResult","isError":true,"content":[{"type":"text","text":"[Command aborted]"}]}}))
            .ev("message_end", assistant(&["finished after cancelling command"]))
            .ev("agent_end", json!({"messages":[{"role":"assistant","stopReason":"stop"}]}))
            .run();
        assert_eq!(result["settled"]["last_event"], "Stop");
        assert_eq!(result["settled"]["interrupted"], false);
        assert_eq!(
            result["settled"]["reply"],
            "finished after cancelling command"
        );

        let result = Feed::new("omp")
            .ev("session_start", json!({"reason":"startup"}))
            .ev("before_agent_start", json!({"prompt":"work"}))
            .ev("agent_end", json!({"messages":[{"role":"assistant","stopReason":"error","errorMessage":"503 Service Unavailable"}]}))
            .at(2.4, "unknown", json!({}))
            .at(0.2, "unknown", json!({}))
            .run();
        assert_eq!(result["steps"][3]["state"]["state"], "working");
        assert_eq!(result["steps"][4]["state"]["state"], "idle");
        assert_eq!(result["settled"]["last_event"], "Stop");
        assert_eq!(result["settled"]["interrupted"], false);
    }

    #[test]
    fn pi_settled_distinguishes_abort_from_error_and_normal_completion() {
        for aborted in [true, false] {
            let result = Feed::new("pi")
                .ev("session_start", json!({"reason":"startup"}))
                .ev("before_agent_start", json!({"prompt":"first"}))
                .ev("message_end", assistant(&["previous reply"]))
                .ev("agent_settled", json!({"aborted":false}))
                .ev("before_agent_start", json!({"prompt":"second"}))
                // pi reported an error, not stopReason=aborted, after Esc during bash.
                .ev("message_end", json!({"message":{"role":"assistant","content":[],"stopReason":"error","errorMessage":"This operation was aborted"}}))
                .ev("agent_settled", json!({"aborted":aborted}))
                .run();
            assert_eq!(result["settled"]["state"], "idle");
            assert_eq!(result["settled"]["interrupted"], aborted);
        }
        let result = Feed::new("pi")
            .ev("session_start", json!({"reason":"startup"}))
            .ev("before_agent_start", json!({"prompt":"first"}))
            .ev("agent_settled", json!({"aborted":true}))
            .ev("before_agent_start", json!({"prompt":"continue"}))
            .ev("message_end", assistant(&["done"]))
            .ev("agent_settled", json!({"aborted":false}))
            .run();
        assert_eq!(result["settled"]["interrupted"], false);
        assert_eq!(result["settled"]["reply"], "done");
    }

    #[test]
    fn pi_and_omp_translations_are_unchanged() {
        let mut cases = json!({});
        cases["pi/turns"] = Feed::new("pi")
            .ev("session_start", json!({"reason":"startup"}))
            .ev("input", json!({"text":"hi"}))
            .ev("before_agent_start", json!({"prompt":"expanded hi"}))
            .ev("tool_execution_start", json!({"toolName":"bash"}))
            .ev("tool_execution_end", json!({"toolName":"bash"}))
            .ev("tool_execution_start", json!({"toolName":"ask"}))
            .ev("ui_prompt_start", json!({}))
            .ev("ui_prompt_end", json!({}))
            .ev("message_end", assistant(&["first", "second"]))
            .ev(
                "message_end",
                json!({"message":{"role":"user","content":[{"type":"text","text":"no"}]}}),
            )
            .ev("agent_settled", json!({}))
            .ev("input", json!({"text":"q1"}))
            .ev("input", json!({"text":"q2","streamingBehavior":"steer"}))
            .ev("input", json!({"text":"q3","streamingBehavior":false}))
            .ev("input", json!({"text":"q4","streamingBehavior":"followUp"}))
            .ev("before_agent_start", json!({"prompt":"x"}))
            .ev("before_agent_start", json!({"prompt":"y"}))
            .ev("before_agent_start", json!({"prompt":"z"}))
            .ev("ui_prompt_start", json!({}))
            .ev("ui_prompt_end", json!({}))
            .with("idle", json!(true))
            .ev("tool_approval_requested", json!({"toolName":"edit"}))
            .ev("tool_approval_resolved", json!({"toolName":"edit"}))
            .ev("something_else", json!({}))
            .ev("session_switch", json!({"reason":"new"}))
            .ev("session_shutdown", json!({"reason":"reload"}))
            .ev("session_shutdown", json!({"reason":"quit"}))
            .run();
        cases["pi/shutdown-without-reason"] = Feed::new("pi")
            .ev("session_start", json!({"reason":"startup"}))
            .ev("session_shutdown", json!({}))
            .run();
        cases["pi/no-ui"] = Feed::new("pi")
            .ev("session_start", json!({"reason":"startup"}))
            .with("has_ui", json!(false))
            .ev("before_agent_start", json!({"prompt":"p"}))
            .with("has_ui", Value::Null)
            .ev("agent_settled", json!({}))
            .with("has_ui", json!(false))
            .run();
        cases["pi/agent-end"] = Feed::new("pi")
            .ev("session_start", json!({"reason":"startup"}))
            .ev("before_agent_start", json!({"prompt":"p"}))
            .ev("message_end", assistant(&["done"]))
            .ev("agent_end", json!({}))
            .at(0.1, "message_end", assistant(&["late"]))
            .at(0.1, "agent_end", json!({}))
            .run();
        cases["omp/turns"] = Feed::new("omp")
            .ev("session_start", json!({"reason":"startup"}))
            .ev("input", json!({"text":"hi"}))
            .ev("before_agent_start", json!({"prompt":"expanded hi"}))
            .ev("agent_start", json!({}))
            .ev("tool_execution_start", json!({"toolName":"ask"}))
            .ev("tool_execution_end", json!({"toolName":"ask"}))
            .ev("tool_execution_start", json!({"toolName":"bash"}))
            .ev("tool_approval_requested", json!({"toolName":"bash"}))
            .ev("tool_approval_resolved", json!({"toolName":"bash"}))
            .ev("tool_execution_end", json!({"toolName":"bash"}))
            .ev("message_end", assistant(&["answer"]))
            .ev("agent_end", json!({"willContinue":true}))
            .ev("agent_end", json!({"messages":[{"role":"user"},{"role":"assistant","stopReason":"error","errorMessage":"503 Service Unavailable"}]}))
            .at(1.0, "agent_start", json!({}))
            .at(0.5, "agent_end", json!({"messages":[{"role":"assistant","stopReason":"error","errorMessage":"bad request"}]}))
            .at(0.1, "message_end", assistant(&["after end"]))
            .at(1.0, "agent_end", json!({"messages":[{"role":"assistant","stopReason":"error","errorMessage":"Rate limit hit"}]}))
            .at(3.0, "ui_prompt_start", json!({}))
            .ev("ui_prompt_end", json!({}))
            .with("idle", json!(true))
            .ev("agent_settled", json!({}))
            .ev("session_switch", json!({"reason":"resume"}))
            .ev("session_shutdown", json!({"reason":"reload"}))
            .run();
        cases["omp/no-ui"] = Feed::new("omp")
            .ev("session_start", json!({"reason":"startup"}))
            .with("has_ui", json!(false))
            .ev("before_agent_start", json!({"prompt":"p"}))
            .with("has_ui", Value::Null)
            .ev("tool_execution_start", json!({"toolName":"ask"}))
            .with("has_ui", json!("yes"))
            .ev("session_start", json!({"reason":"startup"}))
            .ev("tool_execution_start", json!({"toolName":"ask"}))
            .ev("session_shutdown", json!({"reason":"quit"}))
            .run();
        cases["unknown-adapters"] = Feed::new("claude")
            .ev("session_start", json!({"reason":"startup"}))
            .with("adapter", json!("PI"))
            .ev("session_start", json!({"reason":"startup"}))
            .with("adapter", Value::Null)
            .ev("session_start", json!({"reason":"startup"}))
            .with("adapter", json!(""))
            .ev("session_start", json!({"reason":"startup"}))
            .run();
        crate::adapter_snapshot::snapshot("adapters_raw_events.json", &cases);
    }
}
