// From Saddle `crates/corral-core/src/after.rs` at commit `a31dea2`; ranch has changed it since.
//! A record is mutated only while holding its permanent flock inode. Handover
//! requests are separate files; they never transfer authority by PID or signal.
use crate::{Error, Result, cli, now, state};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::{fs::DirBuilderExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

fn id(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err(Error::new(
            1,
            "usage",
            "request_id must contain 1..128 ASCII letters, digits, underscores or hyphens",
        ));
    }
    Ok(())
}
fn directory(name: &str) -> Result<PathBuf> {
    Ok(std::path::absolute(state::dir(name)?.join(".after"))?)
}
fn spawn(exe: &Path, path: &Path) -> Result<()> {
    let mut command = Command::new(exe);
    command
        .arg("__after")
        .arg(path)
        .env("CORRAL_HOME", std::path::absolute(state::home())?)
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command.spawn()?;
    Ok(())
}
pub(crate) fn create(
    name: &str,
    after: &str,
    text: &str,
    force: bool,
    timeout: f64,
    request_id: Option<&str>,
) -> Result<Value> {
    let request_id = request_id
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    id(&request_id)?;
    let target = state::require(name, json!({"op":"status"}))?;
    let other = state::require(after, json!({"op":"status"}))?;
    let dir = directory(name)?;
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&dir)
        .or_else(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                Ok(())
            } else {
                Err(e)
            }
        })?;
    let path = dir.join(format!("{request_id}.json"));
    let lock = state::lock(&path.with_extension("lock"), true)?
        .ok_or_else(|| Error::new(5, "exists", "request_id already owned"))?;
    if path.exists() {
        return Err(Error::new(
            5,
            "exists",
            "request_id already recorded; query corral after",
        ));
    }
    let record = json!({"schema":1,"request_id":request_id,"name":name,"instance":target["instance"],"after":after,"after_instance":other["instance"],"text":text,"force":force,"timeout":timeout,"wait_deadline":now()+timeout.max(0.0),"delivery_deadline":null,"wait_done":false,"phase":"waiting","result":null,"worker":null,"known":false});
    state::write_durable(&path, &record)?;
    drop(lock);
    spawn(&crate::executable()?, &path)?;
    Ok(
        json!({"ok":true,"name":name,"instance":target["instance"],"after":after,"after_instance":other["instance"],"pending":true,"request_id":request_id}),
    )
}
fn records(name: &str) -> Result<Vec<PathBuf>> {
    let dir = directory(name)?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir)? {
        let p = entry?.path();
        if p.extension().is_some_and(|e| e == "json") {
            paths.push(p);
        }
    }
    paths.sort();
    Ok(paths)
}
pub(crate) fn list(name: &str, request_id: Option<&str>) -> Result<Value> {
    if let Some(id_value) = request_id {
        id(id_value)?;
    }
    let records: Vec<_> = records(name)?
        .into_iter()
        .map(state::read)
        .filter(|v| request_id.is_none_or(|id| v["request_id"] == id))
        .collect();
    Ok(
        json!({"ok":true,"name":name,"records":records,"legacy_after":"unrecorded workers are not discoverable or migratable"}),
    )
}
pub(crate) fn names() -> Result<Vec<String>> {
    fn visit(path: &Path, root: &Path, out: &mut Vec<String>) -> Result<()> {
        if path.join(".after").is_dir() {
            let name = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if state::validate(&name).is_ok() {
                out.push(name);
            }
        }
        for e in fs::read_dir(path)? {
            let e = e?;
            if e.file_type()?.is_dir() && !e.file_name().to_string_lossy().starts_with('.') {
                visit(&e.path(), root, out)?;
            }
        }
        Ok(())
    }
    let root = state::home();
    let mut out = Vec::new();
    if root.is_dir() {
        visit(&root, &root, &mut out)?;
    }
    Ok(out)
}
fn terminal(r: &Value) -> bool {
    matches!(
        r["phase"].as_str(),
        Some("confirmed" | "not_delivered" | "unknown" | "expired")
    ) || r["phase"] == "sent" && r["known"] != true
}
fn finish(r: &mut Value, phase: &str, evidence: Value) {
    r["phase"] = json!(phase);
    r["result"] = evidence;
    r["finished_at"] = json!(now());
}
fn wait_step(r: &mut Value) -> Result<()> {
    let result = match cli::status(r["after"].as_str().unwrap()) {
        Err(e) => Some(e.value),
        Ok(st) => {
            let st = st.public;
            let state = st["state"].as_str().unwrap_or("unknown");
            let key = json!([st["state"], st["last_event"], st["last_event_at"]]);
            if st["instance"] != r["after_instance"] {
                Some(json!({"result":"restarted"}))
            } else if state == "unknown" {
                Some(json!({"result":"unknown"}))
            } else if st["paused"] == true {
                // Paused is not finished: wait for it to continue and end its turn.
                r["wait_stable"] = Value::Null;
                None
            } else if matches!(state, "idle" | "blocked") {
                if r["wait_stable"]["key"] == key
                    && now() - r["wait_stable"]["t"].as_f64().unwrap_or(now()) >= 0.5
                {
                    Some(json!({"result":state}))
                } else {
                    if r["wait_stable"]["key"] != key {
                        r["wait_stable"] = json!({"key":key,"t":now()});
                    }
                    None
                }
            } else {
                r["wait_stable"] = Value::Null;
                None
            }
        }
    };
    let result = result.or_else(|| {
        (now() >= r["wait_deadline"].as_f64().unwrap()).then(|| json!({"result":"timeout"}))
    });
    if let Some(result) = result {
        r["wait_result"] = result;
        r["wait_done"] = json!(true);
        r["delivery_deadline"] = json!(now() + r["timeout"].as_f64().unwrap().max(0.0));
    }
    Ok(())
}
fn step(path: &Path, r: &mut Value) -> Result<()> {
    if r["phase"] == "waiting" && r["wait_done"] != true {
        return wait_step(r);
    }
    let name = r["name"].as_str().unwrap().to_owned();
    let instance = r["instance"].as_str().unwrap().to_owned();
    let request_id = r["request_id"].as_str().unwrap().to_owned();
    let text = r["text"].as_str().unwrap().to_owned();
    match r["phase"].as_str().unwrap_or("") {
        "waiting" => {
            let st = cli::status(&name)?;
            if st.public["instance"] != instance {
                finish(r, "not_delivered", json!({"error":"restarted"}));
                return Ok(());
            }
            let protected = r["force"] != true
                && st.pen["last_human_input"]
                    .as_f64()
                    .is_some_and(|t| now() - t < 30.0);
            let paused = st.public["paused"] == true;
            if paused || st.snapshot.is_some() && st.public["state"] != "idle" || protected {
                if now() >= r["delivery_deadline"].as_f64().unwrap() {
                    let error = if paused {
                        "paused"
                    } else if protected {
                        "human_active"
                    } else {
                        "not_idle"
                    };
                    finish(r, "expired", json!({"error":error}));
                }
                return Ok(());
            }
            r["phase"] = json!("sending");
            r["known"] = json!(st.snapshot.is_some());
            r["t0"] = json!(now());
            // This durable fact precedes the single external action.
            state::write_durable(path, r)?;
            match cli::send_once(&name, &text, r["force"] == true, &st, &request_id) {
                Ok(receipt) => {
                    r["receipt"] = receipt;
                    r["phase"] = json!("sent");
                    r["confirm_deadline"] = json!(now() + 15.0);
                    if r["known"] != true {
                        finish(r, "sent", json!({"ok":true,"confirmed":false}));
                    }
                }
                Err(e) if matches!(e.code, 7 | 8 | 10) => {
                    // A definite pre-acceptance refusal is safe to wait on.
                    if now() < r["delivery_deadline"].as_f64().unwrap() {
                        r["phase"] = json!("waiting");
                    } else {
                        finish(r, "expired", e.value);
                    }
                }
                Err(e) => {
                    r["receipt_error"] = e.value; /* resolve sending from pen evidence */
                }
            }
        }
        "sending" => {
            let st = state::require(&name, json!({"op":"status"}))?;
            let evidence = st["recent_sends"]
                .as_array()
                .and_then(|a| a.iter().find(|e| e["request_id"] == request_id));
            if let Some(evidence) = evidence.filter(|_| st["instance"] == instance) {
                r["evidence"] = evidence.clone();
                match evidence["state"].as_str() {
                    Some("written") => {
                        r["phase"] = json!("sent");
                        r["confirm_deadline"] = json!(
                            evidence["written_at"]
                                .as_f64()
                                .unwrap_or(r["t0"].as_f64().unwrap())
                                + 15.0
                        );
                        if r["known"] != true {
                            finish(r, "sent", json!({"ok":true,"confirmed":false}));
                        }
                    }
                    // The RPC receipt may have timed out, but acceptance is
                    // still proven. Keep observing this request; never replay it.
                    Some("accepted") => {}
                    _ => finish(
                        r,
                        "unknown",
                        json!({"error":"write_not_proven","message":"not resending"}),
                    ),
                }
            } else {
                finish(
                    r,
                    "unknown",
                    json!({"error":"no_request_evidence","message":"absence is not proof of non-acceptance; not resending"}),
                );
            }
        }
        "sent" => {
            if let Some(result) =
                cli::confirmation(&name, &text, &instance, r["t0"].as_f64().unwrap())?
            {
                finish(r, "confirmed", result);
            } else if now() >= r["confirm_deadline"].as_f64().unwrap() {
                finish(
                    r,
                    "not_delivered",
                    json!({"error":"not_delivered","message":"no matching input event; not resending"}),
                );
            }
        }
        _ => return Err(Error::new(9, "incompatible", "unsupported after phase")),
    }
    Ok(())
}

pub(crate) fn worker(path: &str) -> i32 {
    let path = Path::new(path);
    let result = (|| -> Result<()> {
        let exe = crate::executable()?;
        let deadline = now() + 45.0;
        let lock = loop {
            if let Some(lock) = state::lock(&path.with_extension("lock"), true)? {
                break lock;
            }
            if now() >= deadline {
                return Err(Error::new(
                    1,
                    "handover_timeout",
                    "previous worker still owns the record",
                ));
            }
            thread::sleep(Duration::from_millis(50));
        };
        let mut r = state::read(path);
        if r["schema"] != 1 {
            return Err(Error::new(9, "incompatible", "unsupported after record"));
        }
        r["worker"] = json!({"exe":exe,"pid":std::process::id()});
        state::write_durable(path, &r)?;
        loop {
            if terminal(&r) {
                return Ok(());
            }
            let handover = state::read(path.with_extension("handover"));
            if let Some(target) = handover["exe"].as_str() {
                if Path::new(target) != exe
                    && !(r["handover"]["state"] == "failed"
                        && r["handover"]["token"] == handover["token"])
                {
                    // Finish our last write before releasing authority. The new
                    // process must acquire this same permanent lock and reread.
                    r["handover"] =
                        json!({"state":"pending","token":handover["token"],"target":target});
                    state::write_durable(path, &r)?;
                    match spawn(Path::new(target), path) {
                        Ok(()) => {
                            drop(lock);
                            return Ok(());
                        }
                        Err(e) => {
                            r["handover"] =
                                json!({"state":"failed","token":handover["token"],"error":e.value});
                            state::write_durable(path, &r)?;
                        }
                    }
                } else if Path::new(target) == exe
                    && (r["handover"]["token"] != handover["token"]
                        || r["handover"]["state"] != "complete")
                {
                    r["handover"] =
                        json!({"state":"complete","token":handover["token"],"target":target});
                    state::write_durable(path, &r)?;
                }
            }
            let old = r.clone();
            if let Err(e) = step(path, &mut r) {
                let phase = if matches!(r["phase"].as_str(), Some("sending" | "sent")) {
                    "unknown"
                } else {
                    "not_delivered"
                };
                finish(&mut r, phase, e.value);
            }
            if r != old {
                state::write_durable(path, &r)?;
            }
            thread::sleep(Duration::from_millis(100));
        }
    })();
    if result.is_ok() { 0 } else { 1 }
}

pub(crate) fn upgrade(name: &str, exe: &Path) -> Result<Vec<Value>> {
    let mut results = Vec::new();
    for path in records(name)? {
        let r = state::read(&path);
        if terminal(&r) {
            results.push(json!({"request_id":r["request_id"],"result":"complete","phase":r["phase"],"worker":"finished"}));
            continue;
        }
        if r["schema"] != 1 {
            results.push(json!({"request_id":r["request_id"],"result":"needs_restart"}));
            continue;
        }
        let token = uuid::Uuid::new_v4().to_string();
        state::write_durable(
            &path.with_extension("handover"),
            &json!({"exe":exe,"token":token}),
        )?;
        // A crashed holder leaves an unlocked inode. Start one replacement; it
        // still rereads under the lock and treats uncertain sending as unknown.
        if let Some(lock) = state::lock(&path.with_extension("lock"), true)? {
            drop(lock);
            spawn(exe, &path)?;
        }
        let deadline = now() + 5.0;
        loop {
            let current = state::read(&path);
            let result = if terminal(&current) {
                "complete"
            } else if current["handover"]["token"] == token
                && current["handover"]["state"] == "failed"
            {
                "failed"
            } else if current["worker"]["exe"] == json!(exe)
                && current["handover"]["token"] == token
                && current["handover"]["state"] == "complete"
            {
                "complete"
            } else {
                "pending"
            };
            if result != "pending" || now() >= deadline {
                results.push(json!({"request_id":r["request_id"],"result":result,"phase":current["phase"],"worker":current["worker"],"handover":current["handover"]}));
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
    }
    Ok(results)
}
