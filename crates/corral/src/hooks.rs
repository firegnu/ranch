// From Saddle `crates/corral-core/src/hooks.rs` at commit `a31dea2`; ranch has changed it since
// (per-agent launch arguments and stop steps moved to `agents` in R3; hooks installed globally
// in R8).
use serde_json::{Value, json};
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};
pub fn run(event: &str) {
    if let Some((kind, event)) = event.split_once(':') {
        let _ = global(kind, event);
        // Some observers require silence, including when they record nothing.
        print!(
            "{}",
            crate::agents::find(kind).map_or("{}\n", |a| a.hook_output())
        );
        return;
    }
    let _ = (|| -> crate::Result<()> {
        let Some(path) = std::env::var_os("CORRAL_EVENTS") else {
            return Ok(());
        };
        let mut bytes = Vec::new();
        std::io::stdin().read_to_end(&mut bytes)?;
        let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        let mut record = json!({"v":1,"t":crate::now(),"ev":event,"inst":std::env::var("CORRAL_INSTANCE").unwrap_or_default(),"has_transcript":body["transcript_path"].as_str().is_some_and(|s|!s.is_empty())});
        for key in [
            "session_id",
            "transcript_path",
            "cwd",
            "source",
            "tool_name",
            "prompt",
            "last_assistant_message",
            "notification_type",
        ] {
            if let Some(v) = body
                .get(key)
                .filter(|v| v.is_string() || v.is_number() || v.is_boolean())
            {
                record[key] = v.clone();
            }
        }
        if let Some(tasks) = body["background_tasks"].as_array() {
            record["background_running"] =
                json!(tasks.iter().filter(|t| t["status"] == "running").count());
        }
        if event == "PermissionRequest"
            && body["tool_name"] == "AskUserQuestion"
            && let Some(questions) = questions(&body["tool_input"]["questions"])
        {
            record["questions"] = questions;
        }
        if event == "PermissionRequest"
            && body["tool_name"] == "Bash"
            && let Some(command) = command(&body["tool_input"]["command"])
        {
            record["command"] = command;
        }
        let mut data = serde_json::to_vec(&record)?;
        data.push(b'\n');
        let mut f = OpenOptions::new()
            .append(true)
            .create(true)
            .mode(0o600)
            .open(path)?;
        f.write_all(&data)?;
        Ok(())
    })();
}

fn command(input: &Value) -> Option<Value> {
    let input = input.as_str()?;
    let mut result = json!({"text":"", "truncated":false});
    let mut remaining = 16 * 1024 - result.to_string().len();
    let mut text = String::new();
    for ch in input.chars() {
        let size = serde_json::to_string(&ch).unwrap().len() - 2;
        let Some(left) = remaining.checked_sub(size) else {
            result["truncated"] = json!(true);
            break;
        };
        remaining = left;
        text.push(ch);
    }
    result["text"] = json!(text);
    Some(result)
}

// PermissionRequest receives the same tool_input as PreToolUse, after any hook updates.
// Only the question fields are retained, never answers or other tool parameters.
pub(crate) fn questions(input: &Value) -> Option<Value> {
    // Count JSON bytes, including escaping and structure, without cloning unbounded strings.
    fn text(input: &Value, remaining: &mut usize, truncated: &mut bool) -> Value {
        let mut out = String::new();
        for ch in input.as_str().unwrap_or_default().chars() {
            let size = serde_json::to_string(&ch).unwrap().len() - 2;
            let Some(left) = remaining.checked_sub(size) else {
                *truncated = true;
                break;
            };
            *remaining = left;
            out.push(ch);
        }
        json!(out)
    }
    let input = input.as_array()?;
    let mut result = json!({"items":[], "truncated":false});
    let mut remaining = 16 * 1024 - result.to_string().len();
    let mut truncated = false;
    let items = result["items"].as_array_mut().unwrap();
    for q in input {
        let mut item = json!({"header":"", "question":"", "multi_select":q["multiSelect"] == true, "options":[]});
        let Some(left) =
            remaining.checked_sub(item.to_string().len() + usize::from(!items.is_empty()))
        else {
            truncated = true;
            break;
        };
        remaining = left;
        item["header"] = text(&q["header"], &mut remaining, &mut truncated);
        item["question"] = text(&q["question"], &mut remaining, &mut truncated);
        let options = item["options"].as_array_mut().unwrap();
        for option in q["options"].as_array().into_iter().flatten() {
            let mut out = json!({"label":"", "description":""});
            let Some(left) =
                remaining.checked_sub(out.to_string().len() + usize::from(!options.is_empty()))
            else {
                truncated = true;
                break;
            };
            remaining = left;
            out["label"] = text(&option["label"], &mut remaining, &mut truncated);
            out["description"] = text(&option["description"], &mut remaining, &mut truncated);
            options.push(out);
        }
        items.push(item);
    }
    result["truncated"] = json!(truncated);
    Some(result)
}
pub(crate) fn helper(target: &Path, switch: bool) -> crate::Result<std::path::PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, symlink};
    let root = std::path::absolute(crate::state::home())?;
    let dir = root.join(".runtime");
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)?;
    let path = dir.join("helper");
    if switch {
        let temp = dir.join(format!("helper.{}.tmp", uuid::Uuid::new_v4()));
        symlink(target, &temp)?;
        if let Err(e) = std::fs::rename(&temp, &path) {
            let _ = std::fs::remove_file(temp);
            return Err(e.into());
        }
    } else if let Err(e) = symlink(target, &path)
        && e.kind() != std::io::ErrorKind::AlreadyExists
    {
        return Err(e.into());
    }
    Ok(path)
}

/// `__hook <kind>:<event>` from a hook file installed globally (`agents::Global`): it records only
/// in the agent corral started, whose parent is the pen, not in one started inside it with the
/// same environment. Fenced Kimi has exactly one runner between it and the pen.
/// The hook runs as the agent's child.
fn global(kind: &str, event: &str) -> crate::Result<()> {
    let (Some(path), Some(adapter)) =
        (std::env::var_os("CORRAL_EVENTS"), crate::agents::find(kind))
    else {
        return Ok(());
    };
    let var = |name| std::env::var(name).unwrap_or_default();
    let pen = var("CORRAL_PEN_PID");
    if var("CORRAL_KIND") != kind
        || [&pen, &var("CORRAL_INSTANCE"), &var("CORRAL_CWD")]
            .iter()
            .any(|v| v.is_empty())
    {
        return Ok(());
    }
    let Ok(pen_pid) = pen.parse::<libc::pid_t>() else {
        return Ok(());
    };
    let fenced_kimi = kind == "kimi" && var("FENCE_RUN") == var("CORRAL_INSTANCE");
    let parent = |pid: libc::pid_t| -> Option<libc::pid_t> {
        #[cfg(target_os = "macos")]
        if fenced_kimi {
            // /bin/ps is setuid on macOS and cannot execute inside the sandbox.
            let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
            let size = std::mem::size_of_val(&info) as i32;
            let read = unsafe {
                libc::proc_pidinfo(
                    pid,
                    libc::PROC_PIDTBSDINFO,
                    0,
                    (&mut info as *mut libc::proc_bsdinfo).cast(),
                    size,
                )
            };
            return (read == size).then_some(info.pbi_ppid as libc::pid_t);
        }
        let out = std::process::Command::new("/bin/ps")
            .args(["-o", "ppid=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse().ok()
    };
    let Some(agent_parent) = parent(unsafe { libc::getppid() }) else {
        return Ok(());
    };
    if agent_parent != pen_pid && (!fenced_kimi || parent(agent_parent) != Some(pen_pid)) {
        return Ok(());
    }
    // Hook input is a small JSON object; anything past the limit is not read.
    let mut bytes = Vec::new();
    std::io::stdin().take(16 << 20).read_to_end(&mut bytes)?;
    let input: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    let mut f = OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o600)
        .open(path)?;
    for mut record in adapter.hook(event, &input) {
        record["v"] = json!(1);
        record["t"] = json!(crate::now());
        record["inst"] = json!(std::env::var("CORRAL_INSTANCE").unwrap_or_default());
        record["cwd"] = json!(std::env::var("CORRAL_CWD").unwrap_or_default());
        let mut data = serde_json::to_vec(&record)?;
        data.push(b'\n');
        f.write_all(&data)?;
    }
    Ok(())
}
