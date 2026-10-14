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
            let p = plan(&g, &home, false)?;
            item["hooks"] = json!("global");
            item["install"] = json!({"path":p.path,"status":p.status,"version":p.version});
            if let Some(enabled) = g.enabled {
                item["install"]["enabled"] = json!(enabled(&home));
            }
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

/// Where each of `g`'s files goes.
fn targets(g: &Global, root: &Path) -> Vec<(PathBuf, &'static str)> {
    g.files
        .iter()
        .map(|(f, body)| {
            (
                if f.is_empty() {
                    root.to_path_buf()
                } else {
                    root.join(f)
                },
                *body,
            )
        })
        .collect()
}

/// What installing (or removing) an agent's global hook file, or directory of them, would do.
struct Plan {
    /// The file or directory.
    path: PathBuf,
    /// `create`, `same` or `overwrite` (`absent` or `remove` when removing), or `foreign` for
    /// anything corral does not own and leaves in place.
    status: &'static str,
    /// The version that is there, if corral's.
    version: Option<u64>,
    /// Which of the files are there, in `Global::files` order.
    present: Vec<bool>,
    /// Temporary files an interrupted install left in corral's directory.
    temps: Vec<PathBuf>,
}

/// Whether `path` is a temporary file corral writes before moving it into place.
fn temporary(path: &Path) -> bool {
    path.file_name()
        .map(|n| n.to_string_lossy())
        .is_some_and(|n| n.starts_with(".corral-") && n.ends_with(".tmp"))
        && fs::symlink_metadata(path).is_ok_and(|m| m.is_file())
}

fn plan(g: &Global, home: &Path, remove: bool) -> Result<Plan> {
    let dir = (g.dir)(home);
    let root = dir.join(g.path);
    let files = targets(g, &root);
    let foreign = Ok(Plan {
        path: root.clone(),
        status: "foreign",
        version: None,
        present: vec![],
        temps: vec![],
    });
    if skills::linked(&root, &dir) || files.iter().any(|(p, _)| skills::linked(p, &dir)) {
        return foreign;
    }
    // A directory is corral's only if it holds nothing but corral's files.
    let mut temps = Vec::new();
    if files.iter().any(|(p, _)| p != &root) {
        match fs::symlink_metadata(&root) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
            Ok(m) if !m.is_dir() => return foreign,
            Ok(_) => {
                for entry in fs::read_dir(&root)? {
                    let entry = entry?.path();
                    if temporary(&entry) {
                        temps.push(entry);
                    } else if !files.iter().any(|(p, _)| p == &entry) {
                        return foreign;
                    }
                }
            }
        }
    }
    let (mut present, mut same, mut version) = (Vec::new(), 0, None);
    for (path, body) in &files {
        match fs::symlink_metadata(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                present.push(false);
                continue;
            }
            Err(e) => return Err(e.into()),
            Ok(m) if !m.is_file() => return foreign,
            Ok(_) => {}
        }
        let text = String::from_utf8(fs::read(path)?).unwrap_or_default();
        let Some(v) = installed_version(&text) else {
            return foreign;
        };
        version = version.or(Some(v));
        present.push(true);
        same += usize::from(text == *body);
    }
    let status = match (present.contains(&true), remove) {
        (false, false) => "create",
        (false, true) if temps.is_empty() => "absent",
        (_, true) => "remove",
        (true, false) if same == files.len() => "same",
        (_, false) => "overwrite",
    };
    Ok(Plan {
        path: root,
        status,
        version,
        present,
        temps,
    })
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
    let p = plan(&g, &home, remove)?;
    let (path, status) = (&p.path, p.status);
    let action = if remove { "uninstall" } else { "install" };
    let mut warnings = Vec::new();
    if status == "foreign" {
        warnings.push(format!("{} is not corral's; left in place", path.display()));
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
        eprint!("确认{verb}？[y/N] ");
        std::io::stderr().flush()?;
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        if !["y", "yes"].contains(&line.trim().to_lowercase().as_str()) {
            return Err(Error::new(1, "declined", "nothing written"));
        }
        // The files may have changed while the user answered.
        let again = plan(&g, &home, remove)?;
        if (again.status, &again.present) != (status, &p.present) {
            return Err(Error::new(
                1,
                "changed",
                format!(
                    "{} changed while waiting for confirmation; nothing written",
                    path.display()
                ),
            ));
        }
    }
    if let Some(warning) = apply(&g, &p, remove)? {
        result["warnings"] = json!([warning]);
    }
    result["written"] = json!(true);
    Ok(result)
}

/// Writes or removes what `p` planned, going by the plan rather than by what is there now: a file
/// planned as new is only linked into place, which fails if one has appeared since. Returns a
/// warning when the directory could not be removed.
fn apply(g: &Global, p: &Plan, remove: bool) -> Result<Option<String>> {
    let path = &p.path;
    let files = targets(g, path);
    if remove {
        // The file that makes the agent load the rest goes first, the directory last.
        for (file, _) in files.iter().rev() {
            match fs::remove_file(file) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                r => r?,
            }
        }
        for temp in &p.temps {
            if temporary(temp) {
                let _ = fs::remove_file(temp);
            }
        }
        if files.iter().any(|(f, _)| f != path)
            && let Err(e) = fs::remove_dir(path)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            return Ok(Some(format!("{} was left in place: {e}", path.display())));
        }
        return Ok(None);
    }
    for ((file, body), present) in files.iter().zip(&p.present) {
        let parent = file.parent().unwrap();
        fs::create_dir_all(parent)?;
        // Written beside the target under a name the agent does not load, then moved over
        // corral's own file, or linked where there was none.
        let temp = parent.join(format!(".corral-{}.tmp", uuid::Uuid::new_v4()));
        let written = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)
            .and_then(|mut f| f.write_all(body.as_bytes()))
            .and_then(|()| {
                if *present {
                    fs::rename(&temp, file)
                } else {
                    fs::hard_link(&temp, file).and_then(|()| fs::remove_file(&temp))
                }
            });
        if let Err(e) = written {
            let _ = fs::remove_file(&temp);
            return Err(e.into());
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file that appears after the plan said there was none is not overwritten (R8 review).
    #[test]
    fn a_file_planned_as_new_is_never_overwritten() {
        for kind in ["amp", "agy"] {
            let home = tempfile::tempdir().unwrap();
            let g = agents::find(kind).unwrap().global().unwrap();
            let p = plan(&g, home.path(), false).unwrap();
            assert_eq!(p.status, "create", "{kind}");
            // The last file, so for agy the first one is already written when it fails.
            let (last, _) = targets(&g, &p.path).pop().unwrap();
            fs::create_dir_all(last.parent().unwrap()).unwrap();
            fs::write(&last, "someone else's\n").unwrap();
            assert!(apply(&g, &p, false).is_err(), "{kind}");
            assert_eq!(fs::read_to_string(&last).unwrap(), "someone else's\n");
            // No temporary file is left beside it, and the result is not corral's.
            let parent = fs::read_dir(last.parent().unwrap()).unwrap();
            assert!(parent.flatten().all(|e| !temporary(&e.path())), "{kind}");
            assert_eq!(plan(&g, home.path(), false).unwrap().status, "foreign");
        }
    }
}
