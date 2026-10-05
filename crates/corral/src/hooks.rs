// From Saddle `crates/corral-core/src/hooks.rs` at commit `a31dea2`, unchanged.
use serde_json::{Value, json};
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};
pub fn run(event: &str) {
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
pub fn known(kind: &str) -> bool {
    matches!(kind, "claude" | "codex" | "pi" | "omp")
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
pub fn argv(argv: &[String], dir: &Path, prompt: Option<&str>) -> crate::Result<Vec<String>> {
    let kind = Path::new(&argv[0])
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let mut out = vec![argv[0].clone()];
    let exe = helper(&crate::executable()?, false)?;
    let helper = shell_words::quote(
        exe.to_str()
            .ok_or_else(|| crate::Error::new(1, "error", "non UTF-8 helper path"))?,
    );
    let command = |ev: &str| format!("{helper} __hook {ev}");
    match kind.as_ref() {
        "claude" => {
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
                let mut h =
                    json!({"hooks":[{"type":"command","command":command(ev),"timeout":10}]});
                if ["PreToolUse", "PostToolUse", "PermissionRequest"].contains(&ev) {
                    h["matcher"] = json!("*")
                }
                hooks[ev] = json!([h]);
            }
            out.extend(["--settings".into(), json!({"hooks":hooks}).to_string()]);
        }
        "codex" => {
            for ev in [
                "SessionStart",
                "UserPromptSubmit",
                "PreToolUse",
                "PostToolUse",
                "PermissionRequest",
                "Stop",
                "Interrupt",
            ] {
                let timeout = if ev == "Interrupt" { 3 } else { 10 };
                out.extend(["-c".into(),format!("hooks.{ev}=[{{hooks=[{{type=\"command\",command={},timeout={timeout}}}]}}]",json!(command(ev)))]);
            }
            out.push("--dangerously-bypass-hook-trust".into());
        }
        "pi" | "omp" => {
            let file = format!("hook_{kind}.ts");
            let body = if kind == "pi" {
                include_str!("../resources/hook_pi.ts")
            } else {
                include_str!("../resources/hook_omp.ts")
            };
            let path = dir.join(file);
            let mut f = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&path)?;
            f.write_all(body.as_bytes())?;
            out.extend(["--extension".into(), path.to_string_lossy().into_owned()]);
        }
        _ => {}
    }
    out.extend_from_slice(&argv[1..]);
    if let Some(p) = prompt {
        out.extend(["--".into(), p.into()]);
    }
    Ok(out)
}
pub fn quit_steps(kind: &str) -> Value {
    match kind {
        "codex" => {
            json!([{"keys":"Aw==","wait":0.3},{"keys":"Aw==","wait":60},{"signal":"TERM","wait":3}])
        }
        "claude" | "pi" | "omp" => json!([{"signal":"HUP","wait":5},{"signal":"TERM","wait":3}]),
        _ => json!([{"signal":"HUP","wait":3},{"signal":"TERM","wait":3}]),
    }
}
