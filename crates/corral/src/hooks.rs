// From Saddle `crates/corral-core/src/hooks.rs` at commit `a31dea2`; ranch has changed it since
// (per-agent launch arguments and stop steps moved to `agents` in R3).
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
