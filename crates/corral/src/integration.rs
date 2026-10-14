//! `corral integration status`: what the adapter registry supports and how this machine stands,
//! for front ends to show. Read-only: it writes nothing and only runs each agent's `--version`.
//! `corral integration install|uninstall` writes or removes an agent's global hook file
//! (`agents::Global`), and nothing else.
use crate::{Error, Result, agents, agents::Global, skills};
use serde_json::{Value, json};
use std::{
    fs,
    io::{IsTerminal, Read, Write},
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

/// How long one agent's `--version` may take before it and its process group are killed.
const VERSION_TIMEOUT: Duration = Duration::from_secs(3);

pub fn status() -> Result<Value> {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let found: Vec<_> = agents::all().map(|a| (a, find(a.kind()))).collect();
    let versions: Vec<Option<String>> = std::thread::scope(|scope| {
        let running: Vec<_> = found
            .iter()
            .map(|(_, path)| scope.spawn(|| path.as_deref().and_then(version)))
            .collect();
        running
            .into_iter()
            .map(|h| h.join().unwrap_or(None))
            .collect()
    });
    let mut list = Vec::new();
    for ((adapter, path), version) in found.iter().zip(versions) {
        let mut places = Vec::new();
        for target in skills::TARGETS {
            if adapter.skill_targets().contains(&target) {
                let (file, status) = skills::plan(target, &home, None, false)?;
                places.push(json!({"target":target,"path":file,"status":status}));
            }
        }
        let mut item = json!({"kind":adapter.kind(),"path":path,"version":version,"hooks":"launch","skills":places});
        if let Some(g) = adapter.global() {
            let (file, status, installed) = plan(&g, &home, false)?;
            item["hooks"] = json!("global");
            item["install"] = json!({"path":file,"status":status,"version":installed});
        }
        list.push(item);
    }
    Ok(json!({"ok":true,"agents":list}))
}

/// The first executable file named `program` in an absolute directory on PATH.
fn find(program: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(program))
        .find(|p| {
            p.metadata()
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}

/// The first non-empty line `program --version` prints, if it succeeds within the timeout.
fn version(program: &Path) -> Option<String> {
    let mut child = Command::new(program)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .ok()?;
    let group = child.id() as libc::pid_t;
    let mut stdout = child.stdout.take()?;
    let (tx, rx) = mpsc::channel();
    // A child the program leaves behind may hold the pipe open, so read on a thread of its own.
    std::thread::spawn(move || {
        let mut out = Vec::new();
        let _ = stdout.read_to_end(&mut out);
        let _ = tx.send(out);
    });
    let deadline = Instant::now() + VERSION_TIMEOUT;
    let out = rx.recv_timeout(VERSION_TIMEOUT).ok();
    let mut status = None;
    while out.is_some() && Instant::now() < deadline {
        if let Ok(Some(s)) = child.try_wait() {
            status = Some(s);
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if status.is_none() {
        // Only the group this call started: the program and whatever it spawned.
        unsafe { libc::kill(-group, libc::SIGKILL) };
        let _ = child.wait();
    }
    if !status?.success() {
        return None;
    }
    String::from_utf8_lossy(&out?)
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_owned)
}

/// The version a global hook file names as `CORRAL_INTEGRATION = <n>`; files without one are not
/// corral's.
fn installed_version(text: &str) -> Option<u64> {
    let rest = &text[text.find("CORRAL_INTEGRATION = ")? + "CORRAL_INTEGRATION = ".len()..];
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// An agent's global hook file and what installing (or removing) it would do there: `create`,
/// `same` or `overwrite` (`absent` or `remove` when removing), or `foreign` for anything corral
/// does not own and leaves in place. Also the version of the file that is there, if corral's.
fn plan(g: &Global, home: &Path, remove: bool) -> Result<(PathBuf, &'static str, Option<u64>)> {
    let dir = (g.dir)(home);
    let path = dir.join(g.file);
    if skills::linked(&path, &dir) {
        return Ok((path, "foreign", None));
    }
    let current = match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
        Ok(m) if !m.is_file() => return Ok((path, "foreign", None)),
        Ok(_) => Some(String::from_utf8(fs::read(&path)?).unwrap_or_default()),
    };
    let version = current.as_deref().and_then(installed_version);
    let status = match (current.as_deref(), version, remove) {
        (None, _, false) => "create",
        (None, _, true) => "absent",
        (Some(_), None, _) => "foreign",
        (Some(_), Some(_), true) => "remove",
        (Some(s), Some(_), false) if s == g.body => "same",
        (Some(_), Some(_), false) => "overwrite",
    };
    Ok((path, status, version))
}

pub fn install(kind: &str, remove: bool, dry: bool, yes: bool) -> Result<Value> {
    let Some(adapter) = agents::find(kind) else {
        return Err(Error::new(1, "usage", "unknown agent kind"));
    };
    let Some(g) = adapter.global() else {
        return Err(Error::new(
            1,
            "usage",
            format!("{kind} needs nothing installed: corral connects its hooks at start"),
        ));
    };
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let (path, status, _) = plan(&g, &home, remove)?;
    let action = if remove { "uninstall" } else { "install" };
    let mut warnings = Vec::new();
    if status == "foreign" {
        warnings.push(format!(
            "{} is not a file corral installed; left in place",
            path.display()
        ));
    }
    let mut result = json!({"ok":true,"action":action,"agent":kind,"path":path,"status":status,"dry_run":dry,"written":false,"warnings":warnings});
    if dry || !matches!(status, "create" | "overwrite" | "remove") {
        return Ok(result);
    }
    if !yes {
        if !std::io::stdin().is_terminal() {
            return Err(Error::new(1, "confirmation_required", format!("integration {action} changes {kind}'s own configuration; confirm interactively or pass --yes after user approval")).with("path", json!(path)).with("status", json!(status)));
        }
        let verb = if remove { "删除" } else { "写入" };
        eprintln!(
            "corral integration {action} 将{verb}：\n  [{status}] {}",
            path.display()
        );
        eprint!("确认{verb}这个文件？[y/N] ");
        std::io::stderr().flush()?;
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        if !["y", "yes"].contains(&line.trim().to_lowercase().as_str()) {
            return Err(Error::new(1, "declined", "nothing written"));
        }
    }
    if remove {
        fs::remove_file(&path)?;
    } else {
        let parent = path.parent().unwrap();
        fs::create_dir_all(parent)?;
        // Written beside the target under a name the agent does not load, then renamed over it.
        let temp = parent.join(format!(".corral-{}.tmp", uuid::Uuid::new_v4()));
        let written = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)
            .and_then(|mut f| f.write_all(g.body.as_bytes()))
            .and_then(|()| fs::rename(&temp, &path));
        if let Err(e) = written {
            let _ = fs::remove_file(&temp);
            return Err(e.into());
        }
    }
    result["written"] = json!(true);
    Ok(result)
}
