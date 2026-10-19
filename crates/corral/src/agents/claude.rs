//! Claude Code. From Saddle `crates/corral-core/src/hooks.rs` at commit `a31dea2` (split per agent
//! in R3); comments from the Python corral `src/corral/agents/claude.py`.
use super::{Adapter, Launch};
use crate::Result;
use serde_json::{Value, json};

pub struct Claude;
impl Adapter for Claude {
    fn kind(&self) -> &'static str {
        "claude"
    }
    /// Hooks go in through `--settings` and apply together with those in the user's own settings.
    fn hook_args(&self, launch: &Launch) -> Result<Vec<String>> {
        let mut hooks = json!({});
        for ev in [
            "SessionStart",
            "UserPromptSubmit",
            "PreToolUse",
            "PostToolUse",
            "PermissionRequest",
            "Notification",
            "Stop",
            "StopFailure",
            "SessionEnd",
        ] {
            let mut h = json!({"hooks":[{"type":"command","command":launch.hook_command(ev),"timeout":10}]});
            if ["PreToolUse", "PostToolUse", "PermissionRequest"].contains(&ev) {
                h["matcher"] = json!("*")
            }
            hooks[ev] = json!([h]);
        }
        Ok(vec![
            "--settings".into(),
            json!({"hooks":hooks}).to_string(),
        ])
    }
    /// Measured: exits normally about a second after SIGHUP.
    fn quit_steps(&self) -> Value {
        json!([{"signal":"HUP","wait":5},{"signal":"TERM","wait":3}])
    }
    fn skill_targets(&self) -> &'static [&'static str] {
        &["claude"]
    }
    fn transcript_interrupt(&self, snapshot: &Value) -> Option<f64> {
        use std::io::{Read, Seek, SeekFrom};
        let input_t = snapshot["turn_started"].as_f64()?;
        let mut file = std::fs::File::open(snapshot["transcript_path"].as_str()?).ok()?;
        let metadata = file.metadata().ok()?;
        if !metadata.is_file() {
            return None;
        }
        // Bound both I/O and allocation, even if Claude appends while we read.
        const LIMIT: u64 = 64 * 1024;
        let start = metadata.len().saturating_sub(LIMIT);
        file.seek(SeekFrom::Start(start)).ok()?;
        let mut bytes = Vec::new();
        file.take(LIMIT).read_to_end(&mut bytes).ok()?;
        let tail = if start > 0 {
            &bytes[bytes.iter().position(|b| *b == b'\n')? + 1..]
        } else {
            &bytes[..]
        };
        // A partial append is not evidence. Try again at the next status poll.
        let tail = tail.strip_suffix(b"\n")?;
        for line in tail.rsplit(|b| *b == b'\n') {
            let record: Value = serde_json::from_slice(line).ok()?;
            match record["type"].as_str()? {
                "assistant" => return None,
                "user" => {
                    if record["sessionId"] != snapshot["main_session"] {
                        return None;
                    }
                    let content = &record["message"]["content"];
                    let text = content.as_str().or_else(|| {
                        let blocks = content.as_array()?;
                        (blocks.len() == 1 && blocks[0]["type"] == "text")
                            .then(|| blocks[0]["text"].as_str())
                            .flatten()
                    })?;
                    if !matches!(
                        text,
                        "[Request interrupted by user]"
                            | "[Request interrupted by user for tool use]"
                    ) {
                        return None;
                    }
                    let t = timestamp(record["timestamp"].as_str()?)?;
                    return (t > input_t).then_some(t);
                }
                _ => {}
            }
        }
        None
    }
}

/// Claude writes UTC ISO timestamps with milliseconds. Reject other formats and invalid dates.
fn timestamp(text: &str) -> Option<f64> {
    if text.len() != 24
        || !text.is_ascii()
        || &text[4..5] != "-"
        || &text[7..8] != "-"
        || &text[10..11] != "T"
        || &text[13..14] != ":"
        || &text[16..17] != ":"
        || &text[19..20] != "."
        || &text[23..24] != "Z"
    {
        return None;
    }
    let number = |a, b| {
        let part = &text[a..b];
        part.bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| part.parse::<i32>().ok())
            .flatten()
    };
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    tm.tm_year = number(0, 4)? - 1900;
    tm.tm_mon = number(5, 7)? - 1;
    tm.tm_mday = number(8, 10)?;
    tm.tm_hour = number(11, 13)?;
    tm.tm_min = number(14, 16)?;
    tm.tm_sec = number(17, 19)?;
    let original = (
        tm.tm_year, tm.tm_mon, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec,
    );
    let seconds = unsafe { libc::timegm(&mut tm) };
    if original
        != (
            tm.tm_year, tm.tm_mon, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec,
        )
    {
        return None;
    }
    Some(seconds as f64 + f64::from(number(20, 23)?) / 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_requires_a_current_explicit_marker_in_the_bounded_tail() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("transcript.jsonl");
        let mut snapshot =
            json!({"main_session":"main", "turn_started":1704067200.0, "transcript_path":path});
        let marker = json!({"type":"user", "sessionId":"main", "timestamp":"2024-01-01T00:00:01.000Z", "message":{"content":[{"type":"text", "text":"[Request interrupted by user]"}]}});
        let write = |records: &[Value]| {
            std::fs::write(
                &path,
                records.iter().map(|r| format!("{r}\n")).collect::<String>(),
            )
            .unwrap();
        };
        write(std::slice::from_ref(&marker));
        assert_eq!(Claude.transcript_interrupt(&snapshot), Some(1704067201.0));
        for input_t in [1704067201.0, 1704067201.0001, 1704067202.0] {
            snapshot["turn_started"] = json!(input_t);
            assert_eq!(Claude.transcript_interrupt(&snapshot), None);
        }
        snapshot["turn_started"] = json!(1704067200.9999);
        assert_eq!(Claude.transcript_interrupt(&snapshot), Some(1704067201.0));
        for timestamp in [
            "unknown",
            "2024-02-30T00:00:01.000Z",
            "2024-01-01T00:00:01Z",
            "2024-01-01T00:00:01.000+00:00",
        ] {
            let mut record = marker.clone();
            record["timestamp"] = json!(timestamp);
            write(&[record]);
            assert_eq!(Claude.transcript_interrupt(&snapshot), None, "{timestamp}");
        }
        let mut next = marker.clone();
        next["message"]["content"] = json!("continue");
        write(&[marker.clone(), next]);
        assert_eq!(Claude.transcript_interrupt(&snapshot), None);
        write(&[marker.clone(), json!({"type":"assistant"})]);
        assert_eq!(Claude.transcript_interrupt(&snapshot), None);
        let mut wrong_session = marker.clone();
        wrong_session["sessionId"] = json!("subagent");
        write(&[wrong_session]);
        assert_eq!(Claude.transcript_interrupt(&snapshot), None);
        // A long older line may straddle the read boundary; only complete tail records count.
        std::fs::write(&path, format!("{}\n{marker}\n", "x".repeat(128 * 1024))).unwrap();
        assert_eq!(Claude.transcript_interrupt(&snapshot), Some(1704067201.0));
        std::fs::write(&path, format!("{marker}\n{}\n", "x".repeat(128 * 1024))).unwrap();
        assert_eq!(Claude.transcript_interrupt(&snapshot), None);
        std::fs::write(&path, format!("{marker}\n{{\"type\":")).unwrap();
        assert_eq!(Claude.transcript_interrupt(&snapshot), None);
    }
}
