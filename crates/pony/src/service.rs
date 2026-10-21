use crate::core::{Error, Result};
use serde_json::{Value, json};
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
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
    manage(&user_home()?, remove, dry || !yes, &mut launchctl)
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
    let commands = if remove {
        if previous.is_some() {
            json!([{ "argv": [LAUNCHCTL, "bootout", target], "when": "apply" }])
        } else {
            json!([])
        }
    } else {
        json!([
            { "argv": [LAUNCHCTL, "bootstrap", domain, path], "when": "apply" },
            { "argv": [LAUNCHCTL, "print", target], "when": "bootstrap_failed" }
        ])
    };
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
        "log": if remove { Value::Null } else { json!({"path": home.join(".pony/logs/launchd.log"), "mode": "0600", "operation": "create_if_missing"}) },
        "commands": commands,
    });
    if dry {
        return Ok(result);
    }
    if remove {
        if previous.is_some() {
            unchanged(home, &previous)?;
            let code = launch(&["bootout".into(), target])?;
            // ESRCH means the owned plist was already unloaded (e.g. a failed bootstrap).
            if code != Some(0) && code != Some(libc::ESRCH) {
                return Err(launch_error("bootout", code));
            }
            unchanged(home, &previous)?;
            fs::remove_file(&path).map_err(|e| io_error(&path, e))?;
        }
    } else {
        unchanged(home, &previous)?;
        prepare_log(home)?;
        if state != "same" {
            write_plist(home, &body, &previous)?;
        }
        let code = launch(&[
            "bootstrap".into(),
            domain,
            path.to_string_lossy().into_owned(),
        ])?;
        // bootstrap reports an error for an already loaded job; only a successful
        // print of this exact service turns that error into an idempotent success.
        if code != Some(0) && launch(&["print".into(), target])? != Some(0) {
            return Err(launch_error("bootstrap", code));
        }
    }
    result["applied"] = true.into();
    Ok(result)
}

fn launch_error(action: &str, code: Option<i32>) -> Error {
    Error::new(
        "launchctl_failed",
        format!("launchctl {action} failed (exit {code:?}); plist left in place"),
    )
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
