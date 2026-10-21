use crate::core::{Error, Result};
use serde_json::{Value, json};
use std::{
    env,
    ffi::{CStr, OsStr},
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const LABEL: &str = "local.ranch.pony";
const MARKER: &str = "<!-- PONY_SERVICE = 1 -->";
const LAUNCHCTL: &str = "/bin/launchctl";

fn user_home() -> Result<PathBuf> {
    let home = env::var_os("HOME")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Error::new("invalid_data", "HOME is not set"))?;
    std::path::absolute(home).map_err(|_| Error::new("invalid_data", "cannot resolve HOME"))
}

fn plist(home: &Path) -> PathBuf {
    home.join("Library/LaunchAgents/local.ranch.pony.plist")
}

fn io_error(path: &Path, error: io::Error) -> Error {
    Error::new("io", format!("{}: {error}", path.display()))
}

fn foreign(path: &Path) -> Error {
    Error::new(
        "foreign",
        format!("{} is not pony's; left in place", path.display()),
    )
}

// HOME itself may be a symlink (dotfiles); nothing below it in the service path may be.
fn read_owned(home: &Path) -> Result<Option<Vec<u8>>> {
    for directory in [home.join("Library"), home.join("Library/LaunchAgents")] {
        match fs::symlink_metadata(&directory) {
            Ok(metadata) if !metadata.is_dir() => return Err(foreign(&directory)),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(io_error(&directory, error)),
        }
    }
    let path = plist(home);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if !metadata.is_file() => return Err(foreign(&path)),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error(&path, error)),
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(&path)
        .map_err(|error| io_error(&path, error))?;
    if !file.metadata().map_err(|e| io_error(&path, e))?.is_file() {
        return Err(foreign(&path));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|e| io_error(&path, e))?;
    if !std::str::from_utf8(&bytes).is_ok_and(|s| s.lines().any(|line| line == MARKER)) {
        return Err(foreign(&path));
    }
    Ok(Some(bytes))
}

pub fn status() -> &'static str {
    if !cfg!(target_os = "macos") {
        "unsupported"
    } else if user_home()
        .and_then(|home| read_owned(&home))
        .is_ok_and(|p| p.is_some())
    {
        "installed"
    } else {
        "not_installed"
    }
}

fn xml_path(path: &Path) -> Result<String> {
    let text = path
        .to_str()
        .filter(|s| {
            s.chars().all(|c| {
                matches!(c, '\t' | '\n' | '\r') || c >= ' ' && c != '\u{fffe}' && c != '\u{ffff}'
            })
        })
        .ok_or_else(|| Error::new("invalid_data", "service paths must be valid XML text"))?;
    Ok(text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;"))
}

fn content(home: &Path) -> Result<String> {
    let program = xml_path(&home.join(".local/bin/pony"))?;
    let log = xml_path(&home.join(".pony/logs/launchd.log"))?;
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
{MARKER}
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array><string>{program}</string><string>run</string></array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>
    <key>ThrottleInterval</key><integer>10</integer>
    <key>StandardOutPath</key><string>{log}</string>
    <key>StandardErrorPath</key><string>{log}</string>
</dict>
</plist>
"#
    ))
}

pub fn run(remove: bool, dry: bool, yes: bool) -> Result<Value> {
    if !cfg!(target_os = "macos") {
        return Err(Error::new("unsupported", "pony service requires macOS"));
    }
    let home = user_home()?;
    let dry = dry || !yes;
    if !dry {
        check_context(&home, unsafe { libc::geteuid() }, account_home)?;
    }
    manage(&home, remove, dry, &mut launchctl)
}

fn check_context(
    home: &Path,
    uid: libc::uid_t,
    lookup: impl FnOnce(libc::uid_t) -> Result<PathBuf>,
) -> Result<()> {
    if uid == 0 {
        return Err(Error::new(
            "unsafe_context",
            "service changes require a non-root user; do not use sudo",
        ));
    }
    let account = lookup(uid)?;
    let same = fs::metadata(home)
        .ok()
        .zip(fs::metadata(account).ok())
        .is_some_and(|(a, b)| a.is_dir() && b.is_dir() && a.dev() == b.dev() && a.ino() == b.ino());
    if !same {
        return Err(Error::new(
            "unsafe_context",
            "HOME must name the current user's account home directory; preview is still available",
        ));
    }
    Ok(())
}

fn account_home(uid: libc::uid_t) -> Result<PathBuf> {
    let mut size = usize::try_from(unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) })
        .unwrap_or(16384)
        .max(1024);
    loop {
        let mut buffer = vec![0u8; size];
        let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
        let mut found = std::ptr::null_mut();
        let code = unsafe {
            libc::getpwuid_r(
                uid,
                &mut entry,
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut found,
            )
        };
        if code == libc::ERANGE {
            size = size.checked_mul(2).ok_or_else(|| {
                Error::new("unsafe_context", "account lookup buffer is too large")
            })?;
            continue;
        }
        if code != 0 || found.is_null() || entry.pw_dir.is_null() {
            return Err(Error::new(
                "unsafe_context",
                "could not determine the current user's account home directory",
            ));
        }
        // getpwuid_r keeps the path in buffer; copy it before the buffer is dropped.
        return Ok(PathBuf::from(OsStr::from_bytes(
            unsafe { CStr::from_ptr(entry.pw_dir) }.to_bytes(),
        )));
    }
}

fn launchctl(args: &[String]) -> Result<Option<i32>> {
    Command::new(LAUNCHCTL)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.code())
        .map_err(|_| Error::new("launchctl_failed", "could not execute /bin/launchctl"))
}

fn manage(
    home: &Path,
    remove: bool,
    dry: bool,
    launch: &mut impl FnMut(&[String]) -> Result<Option<i32>>,
) -> Result<Value> {
    let path = plist(home);
    let previous = read_owned(home)?;
    let body = content(home)?;
    let state = match (remove, &previous) {
        (true, None) => "absent",
        (true, Some(_)) => "remove",
        (false, None) => "create",
        (false, Some(bytes)) if bytes == body.as_bytes() => "same",
        (false, Some(_)) => "replace",
    };
    let domain = format!("gui/{}", unsafe { libc::geteuid() });
    let target = format!("{domain}/{LABEL}");
    let mut commands = Vec::new();
    if (remove && previous.is_some()) || state == "replace" {
        commands.push(json!({ "argv": [LAUNCHCTL, "bootout", target], "when": "apply" }));
        commands.push(json!({ "argv": [LAUNCHCTL, "print", target], "when": "bootout_in_progress", "interval_seconds": 1, "timeout_seconds": 30 }));
    }
    if !remove {
        commands.extend([
            json!({ "argv": [LAUNCHCTL, "bootstrap", domain, path], "when": "apply" }),
            json!({ "argv": [LAUNCHCTL, "print", target], "when": "bootstrap_failed" }),
        ]);
    }
    let mut result = json!({
        "ok": true, "action": if remove { "uninstall" } else { "install" },
        "dry_run": dry, "applied": false, "path": path, "status": state,
        "content": if remove { None } else { Some(&body) },
        "directories": if remove { vec![] } else { vec![
            json!({"path": home.join("Library"), "mode": "0755", "operation": "create_if_missing"}),
            json!({"path": home.join("Library/LaunchAgents"), "mode": "0755", "operation": "create_if_missing"}),
            json!({"path": home.join(".pony"), "mode": "0700", "operation": "create_if_missing"}),
            json!({"path": home.join(".pony/logs"), "mode": "0700", "operation": "create_if_missing"}),
        ] },
        "log": if remove { Value::Null } else { json!({"path": home.join(".pony/logs/launchd.log"), "mode": "0600", "operation": "create_if_missing", "restrict_if_wider": true}) },
        "commands": commands,
    });
    if dry {
        return Ok(result);
    }
    if !remove {
        prepare_log(home)?;
    }
    unchanged(home, &previous)?;
    let unloaded = (remove && previous.is_some()) || state == "replace";
    if unloaded {
        let deadline = Instant::now() + Duration::from_secs(30);
        bootout(&target, launch, &mut || {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            std::thread::sleep(remaining.min(Duration::from_secs(1)));
            true
        })?;
    }
    let applied = (|| {
        if remove {
            if previous.is_some() {
                unchanged(home, &previous)?;
                fs::remove_file(&path).map_err(|e| io_error(&path, e))?;
            }
        } else {
            unchanged(home, &previous)?;
            if state != "same" {
                write_plist(home, &body, &previous)?;
            }
            let code = launch(&[
                "bootstrap".into(),
                domain,
                path.to_string_lossy().into_owned(),
            ])?;
            if code != Some(0) {
                let loaded = launch(&["print".into(), target])? == Some(0);
                if loaded && state == "create" {
                    // Restore the absent-file state so a retry cannot mistake our new
                    // file plus somebody else's loaded definition for a same install.
                    unchanged(home, &Some(body.as_bytes().to_vec()))?;
                    fs::remove_file(&path).map_err(|e| io_error(&path, e))?;
                    return Err(Error::new(
                        "already_loaded",
                        "the service name is already loaded from elsewhere; new plist removed; use launchctl bootout or log out before installing again",
                    ));
                }
                if !loaded || state != "same" {
                    return Err(launch_error("bootstrap", code));
                }
            }
        }
        Ok(())
    })();
    applied.map_err(|mut error: Error| {
        if unloaded {
            error.message.push_str(if remove {
                "; service was unloaded; plist was not removed; resolve the error and rerun pony service uninstall --yes"
            } else {
                "; previous service was unloaded; new service was not confirmed loaded; resolve the error and rerun pony service install --yes"
            });
        }
        error
    })?;
    result["applied"] = true.into();
    Ok(result)
}

fn launch_error(action: &str, code: Option<i32>) -> Error {
    let exit = code.map_or_else(
        || "terminated by signal".into(),
        |code| format!("exit {code}"),
    );
    Error::new(
        "launchctl_failed",
        format!("launchctl {action} failed ({exit}); plist left in place"),
    )
}

fn bootout(
    target: &str,
    launch: &mut impl FnMut(&[String]) -> Result<Option<i32>>,
    wait: &mut impl FnMut() -> bool,
) -> Result<()> {
    match launch(&["bootout".into(), target.into()])? {
        Some(libc::ESRCH) => return Ok(()),
        Some(0) | Some(libc::EINPROGRESS) => {}
        code => return Err(launch_error("bootout", code)),
    }
    loop {
        let code = launch(&["print".into(), target.into()]).map_err(|mut error| {
            error.message.push_str("; stop requested but completion is unknown; plist left unchanged; retry this command shortly");
            error
        })?;
        match code {
            Some(113) => return Ok(()), // launchctl: service not found.
            Some(0) => {}
            code => {
                let mut error = launch_error("print", code);
                error.message.push_str(
                    "; stop requested but completion is unknown; retry this command shortly",
                );
                return Err(error);
            }
        }
        if !wait() {
            return Err(Error::new(
                "stop_pending",
                "service is still stopping after 30 seconds; plist left unchanged; retry this command shortly",
            ));
        }
    }
}

fn unchanged(home: &Path, previous: &Option<Vec<u8>>) -> Result<()> {
    if &read_owned(home)? != previous {
        return Err(Error::new(
            "changed",
            "service plist changed; not overwritten or removed",
        ));
    }
    Ok(())
}

fn write_plist(home: &Path, body: &str, previous: &Option<Vec<u8>>) -> Result<()> {
    unchanged(home, previous)?;
    let path = plist(home);
    let directory = path.parent().unwrap();
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o755)
        .create(directory)
        .map_err(|e| io_error(directory, e))?;
    unchanged(home, previous)?;
    let temporary = directory.join(format!(
        ".pony-{}-{}.tmp",
        std::process::id(),
        crate::core::now().to_bits()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .open(&temporary)
        .map_err(|e| io_error(&temporary, e))?;
    let result = (|| {
        file.write_all(body.as_bytes())
            .map_err(|e| io_error(&temporary, e))?;
        file.sync_all().map_err(|e| io_error(&temporary, e))?;
        unchanged(home, previous)?;
        if previous.is_some() {
            fs::rename(&temporary, &path).map_err(|e| io_error(&path, e))?;
        } else {
            // Never overwrite a file that appeared after planning a new install.
            fs::hard_link(&temporary, &path).map_err(|e| io_error(&path, e))?;
        }
        File::open(directory)
            .and_then(|f| f.sync_all())
            .map_err(|e| io_error(directory, e))
    })();
    let _ = fs::remove_file(&temporary);
    result
}

fn prepare_log(home: &Path) -> Result<()> {
    for path in [home.join(".pony"), home.join(".pony/logs")] {
        match fs::DirBuilder::new().mode(0o700).create(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(io_error(&path, error)),
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)
            .map_err(|e| io_error(&path, e))?;
        check_private(&file, &path, true)?;
    }
    let path = home.join(".pony/logs/launchd.log");
    let file = OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(&path)
        .map_err(|e| io_error(&path, e))?;
    let metadata = file.metadata().map_err(|e| io_error(&path, e))?;
    let mode = metadata.mode() & 0o7777;
    if metadata.is_file()
        && metadata.uid() == unsafe { libc::geteuid() }
        && mode & 0o600 == 0o600
        && mode != 0o600
    {
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|e| io_error(&path, e))?;
    }
    check_private(&file, &path, false)
}

fn check_private(file: &File, path: &Path, directory: bool) -> Result<()> {
    let metadata = file.metadata().map_err(|e| io_error(path, e))?;
    if metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o7777 != if directory { 0o700 } else { 0o600 }
        || if directory {
            !metadata.is_dir()
        } else {
            !metadata.is_file()
        }
    {
        return Err(Error::new(
            "unsafe_path",
            format!(
                "{}: expected current-user-owned private directory (0700) or regular file (0600)",
                path.display()
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
