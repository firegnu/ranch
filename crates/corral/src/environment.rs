// From Saddle `crates/corral-core/src/environment.rs` at commit `a31dea2`, unchanged.
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::Read,
    os::unix::process::CommandExt,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
const KEYS: &[&str] = &[
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "TMPDIR",
    "LANG",
    "SSH_AUTH_SOCK",
];
pub fn build(
    name: &str,
    instance: &str,
    events: &std::path::Path,
    extra: &Value,
) -> (BTreeMap<String, String>, Vec<String>) {
    let account = unsafe {
        libc::getpwuid(libc::getuid()).as_ref().map(|p| {
            let read = |s: *const libc::c_char| {
                if s.is_null() {
                    String::new()
                } else {
                    std::ffi::CStr::from_ptr(s).to_string_lossy().into_owned()
                }
            };
            (read(p.pw_dir), read(p.pw_name), read(p.pw_shell))
        })
    };
    let shell = std::env::var("SHELL")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            account
                .as_ref()
                .map(|p| p.2.clone())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "/bin/sh".into());

    let mut env: BTreeMap<String, String> = std::env::vars()
        .filter(|(k, _)| KEYS.contains(&k.as_str()) || k.starts_with("LC_"))
        .collect();
    if let Some((home, user, _)) = &account {
        for (key, value) in [("HOME", home), ("USER", user), ("LOGNAME", user)] {
            if env.get(key).is_none_or(|s| s.is_empty()) {
                env.insert(key.into(), value.clone());
            }
        }
    }
    env.insert("SHELL".into(), shell.clone());

    env.insert("PATH".into(), "/usr/bin:/bin:/usr/sbin:/sbin".into());
    let mut c = Command::new(&shell);
    c.args([
        "-l",
        "-i",
        "-c",
        "printf '\\n__CORRAL_ENV_BEGIN__\\n'; /usr/bin/env -0; printf '\\n__CORRAL_ENV_END__\\n'",
    ])
    .env_clear()
    .envs(&env)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
    if let Some(h) = env.get("HOME") {
        c.current_dir(h);
    }
    unsafe {
        c.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let captured = capture(&mut c, Duration::from_secs(10));
    let mut warnings = Vec::new();
    let mut captured_env = None;
    if let Some(bytes) = captured {
        let begin = b"\n__CORRAL_ENV_BEGIN__\n";
        let end = b"\n__CORRAL_ENV_END__\n";
        if let Some(i) = bytes.windows(begin.len()).position(|b| b == begin) {
            let rest = &bytes[i + begin.len()..];
            if let Some(j) = rest.windows(end.len()).position(|b| b == end) {
                let pairs: BTreeMap<String, String> = rest[..j]
                    .split(|b| *b == 0)
                    .filter_map(|s| {
                        let i = s.iter().position(|b| *b == b'=')?;
                        Some((
                            String::from_utf8_lossy(&s[..i]).into_owned(),
                            String::from_utf8_lossy(&s[i + 1..]).into_owned(),
                        ))
                    })
                    .collect();
                if !pairs.is_empty() {
                    captured_env = Some(pairs);
                }
            }
        }
    }
    if let Some(e) = captured_env {
        env = e
    } else {
        warnings.push(format!(
            "cannot capture login shell {shell}; using a minimal environment instead"
        ));
        for (k, v) in std::env::vars() {
            if KEYS.contains(&k.as_str())
                || k == "PATH"
                || k.starts_with("LC_")
                || ["http_proxy", "https_proxy", "all_proxy", "no_proxy"]
                    .contains(&k.to_lowercase().as_str())
            {
                env.insert(k, v);
            }
        }
    }
    env.retain(|k, _| {
        (!k.starts_with("CORRAL_") && (!k.starts_with("CODEX_") || k == "CODEX_HOME"))
            && ![
                "SHLVL",
                "PWD",
                "OLDPWD",
                "_",
                "CLAUDECODE",
                "CLAUDE_PID",
                "CLAUDE_CODE_ENTRYPOINT",
                "CLAUDE_CODE_SESSION_ID",
                "CLAUDE_CODE_CHILD_SESSION",
                "CLAUDE_CODE_SESSION_ATTENDED",
                "CLAUDE_CODE_MESSAGING_SOCKET",
                "CLAUDE_CODE_MESSAGING_TOKEN",
                "CLAUDE_CODE_EXECPATH",
                "TMUX",
                "TMUX_PANE",
                "STY",
                "WINDOW",
                "ZELLIJ",
                "ZELLIJ_SESSION_NAME",
                "ZELLIJ_PANE_ID",
                "TERM_SESSION_ID",
                "ITERM_SESSION_ID",
                "KITTY_WINDOW_ID",
                "KITTY_PID",
                "KITTY_LISTEN_ON",
                "WEZTERM_PANE",
                "WEZTERM_UNIX_SOCKET",
                "TERM_PROGRAM",
                "TERM_PROGRAM_VERSION",
            ]
            .contains(&k.as_str())
    });
    env.insert("TERM".into(), "xterm-256color".into());
    env.insert("COLORTERM".into(), "truecolor".into());
    if let Some(e) = extra.as_object() {
        for (k, v) in e {
            if let Some(s) = v.as_str() {
                env.insert(k.clone(), s.into());
            }
        }
    }
    for (k, v) in [
        ("CORRAL_NAME", json!(name)),
        ("CORRAL_INSTANCE", json!(instance)),
        ("CORRAL_EVENTS", json!(events)),
        ("CORRAL_HOME", json!(crate::state::home())),
    ] {
        env.insert(k.into(), v.as_str().unwrap_or("").into());
    }
    (env, warnings)
}

// Include pipe EOF in the deadline: login files can launch children that retain stdout.
fn capture(c: &mut Command, timeout: Duration) -> Option<Vec<u8>> {
    use std::os::fd::AsRawFd;
    let mut child = c.spawn().ok()?;
    let result = (|| {
        let mut out = child.stdout.take()?;
        if unsafe { libc::fcntl(out.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) } < 0 {
            return None;
        }
        let mut bytes = Vec::new();
        let mut eof = false;
        let until = Instant::now() + timeout;
        loop {
            if !eof {
                let mut buf = [0; 65536];
                match out.read(&mut buf) {
                    Ok(0) => eof = true,
                    Ok(n) => bytes.extend_from_slice(&buf[..n]),
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                        ) => {}
                    Err(_) => return None,
                }
            }
            if child.try_wait().ok()?.is_some() && eof {
                return Some(bytes);
            }
            if Instant::now() >= until {
                return None;
            }
            thread::sleep(Duration::from_millis(5));
        }
    })();
    if result.is_none() {
        unsafe {
            libc::kill(-(child.id() as i32), libc::SIGKILL);
        }
        let _ = child.wait();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn login_capture_deadline_includes_inherited_pipe_writers() {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "sleep 0.5 & exit 0"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let before = Instant::now();
        let captured = capture(&mut command, Duration::from_millis(100));
        assert!(
            before.elapsed() < Duration::from_millis(400),
            "capture waited past deadline for a descendant pipe"
        );
        assert!(
            captured.is_none(),
            "timed out environment is not a successful capture"
        );
    }
}
