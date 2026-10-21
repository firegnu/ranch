use serde_json::{Value, json};
mod support;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};
use std::{
    process::{Child, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Fixture(tempfile::TempDir, RejectingProxy);

// Production endpoints stay fixed. A local CONNECT proxy rejects every request,
// so CLI tests cannot send even fake credentials to Slack or any external server.
struct RejectingProxy {
    address: std::net::SocketAddr,
    stopped: std::sync::Arc<std::sync::atomic::AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl RejectingProxy {
    fn new() -> Self {
        use std::io::{Read, Write};
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = stopped.clone();
        let worker = thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    if stream.read_exact(&mut byte).is_err() {
                        break;
                    }
                    header.push(byte[0]);
                }
                let _ = stream.write_all(
                    b"HTTP/1.1 502 Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
            }
        });
        Self {
            address,
            stopped,
            worker: Some(worker),
        }
    }
}
impl Drop for RejectingProxy {
    fn drop(&mut self) {
        self.stopped
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = std::net::TcpStream::connect(self.address);
        self.worker.take().unwrap().join().unwrap();
    }
}

impl Fixture {
    fn new() -> Self {
        Self(tempfile::tempdir().unwrap(), RejectingProxy::new())
    }

    fn home(&self) -> PathBuf {
        self.0.path().join("pony")
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_pony"));
        command
            .env_clear()
            .env("HOME", self.0.path())
            .env("PONY_HOME", self.home())
            .env("CORRAL_HOME", self.0.path().join("corral"))
            .env("PATH", "/usr/bin:/bin")
            .current_dir(self.0.path());
        command
    }

    fn call(&self, args: &[&str], code: i32) -> Value {
        let output = self.command().args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(code), "{output:?}");
        decode(&output)
    }

    fn configured() -> Self {
        let fixture = Self::new();
        support::fake_corral(fixture.0.path());
        fs::create_dir(fixture.home()).unwrap();
        fs::set_permissions(fixture.home(), fs::Permissions::from_mode(0o700)).unwrap();
        fs::create_dir(fixture.home().join("secrets")).unwrap();
        fs::set_permissions(
            fixture.home().join("secrets"),
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        fixture.write_config(CONFIG);
        let secret = fixture.home().join("secrets/slack.toml");
        fs::write(
            &secret,
            "app_token = 'xapp-test'\nbot_token = 'xoxb-test'\n",
        )
        .unwrap();
        fs::set_permissions(secret, fs::Permissions::from_mode(0o600)).unwrap();
        fixture
    }

    fn write_config(&self, text: &str) {
        fs::write(
            self.home().join("config.toml"),
            format!(
                "proxy = 'http://{}'\ncorral = '{}'\n{text}",
                self.1.address,
                self.0.path().join("corral-test").display()
            ),
        )
        .unwrap();
    }

    fn assert_private(&self, values: &[Value]) {
        let mut output = values.iter().map(Value::to_string).collect::<String>();
        for file in ["run/status.json", "logs/pony.log"] {
            output.push_str(&fs::read_to_string(self.home().join(file)).unwrap_or_default());
        }
        for private in ["xapp-test", "xoxb-test", "U0123456789"] {
            assert!(!output.contains(private), "private data appeared in output");
        }
    }
}

fn decode(output: &Output) -> Value {
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_eq!(
        output.stdout.iter().filter(|&&b| b == b'\n').count(),
        1,
        "{output:?}"
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn missing_config_status_is_read_only_and_matches_contract() {
    let fixture = Fixture::new();
    assert_eq!(
        fixture.call(&["--version"], 0),
        json!({"ok": true, "version": "0.1.0", "contract": "1"})
    );
    let status = fixture.call(&["status"], 0);
    assert_eq!(status["ok"], true);
    assert_eq!(status["contract"], "1");
    assert_eq!(status["version"], "0.1.0");
    assert_eq!(status["config"], "missing");
    assert!(
        status["config_message"]
            .as_str()
            .unwrap()
            .contains("config.toml")
    );
    assert_eq!(
        status["service"],
        if cfg!(target_os = "macos") {
            "not_installed"
        } else {
            "unsupported"
        }
    );
    assert_eq!(status["running"], false);
    for key in ["pid", "started", "last_exit"] {
        assert!(status[key].is_null(), "{status}");
    }
    assert_eq!(status["chats"], json!([]));
    assert!(!fixture.home().exists());
    assert!(!fixture.0.path().join("corral").exists());
    assert_eq!(fixture.call(&["status", "extra"], 2)["error"], "usage");
    assert_eq!(fs::read_dir(fixture.0.path()).unwrap().count(), 0);
}

const CONFIG: &str =
    "[slack]\nowner = 'U0123456789'\nchannels = [{ id = 'C0AAAAAAAAA', repo = 'ranch' }]\n";

#[test]
fn config_errors_are_located_and_valid_stopped_config_is_off() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.home()).unwrap();
    fs::set_permissions(fixture.home(), fs::Permissions::from_mode(0o700)).unwrap();
    for config in [
        "[slack",
        "unknown = true",
        "corral = 'relative/path'",
        "proxy = 42",
        "[slack]\nchannels = []",
        "[slack]\nowner = 'U0123456789'",
        "[slack]\nowner = 'private-owner'\nchannels = []",
        "[slack]\nowner = 'U0123456789'\nchannels = [{ id = '#name', repo = 'ranch' }]",
        "[slack]\nowner = 'U0123456789'\nchannels = [{ id = 'C0AAAAAAAAA', repo = '../ranch' }]",
        "[slack]\nowner = 'U0123456789'\nchannels = [{ id = 'C0AAAAAAAAA', repo = 'ranch' }, { id = 'C0AAAAAAAAA', repo = 'other' }]",
        "[slack]\nowner = 'U0123456789'\nchannels = [{ id = 'C0AAAAAAAAA', repo = 'ranch' }, { id = 'G0BBBBBBBBB', repo = 'ranch' }]",
    ] {
        fs::write(fixture.home().join("config.toml"), config).unwrap();
        let status = fixture.call(&["status"], 0);
        assert_eq!(status["config"], "invalid", "{config}: {status}");
        let message = status["config_message"].as_str().unwrap();
        assert!(message.contains("config.toml:"), "{message}");
        assert!(!message.contains("U0123456789"));
        assert!(!message.contains("private-owner"));
        assert_eq!(status["running"], false);
    }
    fs::write(fixture.home().join("config.toml"), CONFIG).unwrap();
    let status = fixture.call(&["status"], 0);
    assert_eq!(status["config"], "ok");
    assert!(status["config_message"].is_null());
    assert_eq!(status["chats"].as_array().unwrap().len(), 1);
    let chat = &status["chats"][0];
    assert_eq!(chat["kind"], "slack");
    assert_eq!(chat["connection"], "off");
    assert!(chat["since"].is_null());
    for key in ["reason", "message", "workspace", "bot"] {
        assert!(chat[key].is_null(), "{status}");
    }
    assert_eq!(chat["other_connections"], 0);
    assert_eq!(
        chat["channels"],
        json!([{"id": "C0AAAAAAAAA", "repo": "ranch", "name": null, "private": null, "problem": null}])
    );
    assert!(!status.to_string().contains("U0123456789"));
    assert_eq!(fs::read_dir(fixture.home()).unwrap().count(), 1);
}

#[test]
fn run_without_config_logs_once_and_exits_successfully() {
    let fixture = Fixture::new();
    assert_eq!(fixture.call(&["run"], 0)["ok"], true);
    let log = fs::read_to_string(fixture.home().join("logs/pony.log")).unwrap();
    assert_eq!(log.lines().count(), 1);
    assert!(log.contains("config_missing"));
    assert_eq!(fixture.call(&["status"], 0)["running"], false);
}

#[test]
fn unsafe_credentials_are_rejected_without_leaking_values() {
    for case in [
        "file_mode",
        "directory_mode",
        "file_link",
        "directory_link",
        "not_regular",
    ] {
        let fixture = Fixture::configured();
        let secret = fixture.home().join("secrets/slack.toml");
        let secrets = fixture.home().join("secrets");
        match case {
            "file_mode" => fs::set_permissions(&secret, fs::Permissions::from_mode(0o644)).unwrap(),
            "directory_mode" => {
                fs::set_permissions(&secrets, fs::Permissions::from_mode(0o755)).unwrap()
            }
            "file_link" => {
                fs::rename(&secret, fixture.0.path().join("elsewhere.toml")).unwrap();
                symlink(fixture.0.path().join("elsewhere.toml"), &secret).unwrap();
            }
            "directory_link" => {
                fs::rename(&secrets, fixture.0.path().join("elsewhere")).unwrap();
                symlink(fixture.0.path().join("elsewhere"), &secrets).unwrap();
            }
            "not_regular" => {
                fs::remove_file(&secret).unwrap();
                fs::create_dir(&secret).unwrap();
            }
            _ => unreachable!(),
        }
        let run = fixture.call(&["run"], 1);
        assert_eq!(run["error"], "secrets_too_open", "{case}: {run}");
        let status = fixture.call(&["status"], 0);
        assert_eq!(status["config"], "invalid", "{case}: {status}");
        assert_eq!(status["running"], false);
        assert_eq!(status["last_exit"]["code"], "secrets_too_open");
        assert_eq!(status["chats"][0]["connection"], "off");
        assert!(status["chats"][0]["reason"].is_null());
        fixture.assert_private(&[run, status]);
    }
    for case in ["home_mode", "home_link", "home_link_slash", "home_link_dot"] {
        let fixture = Fixture::configured();
        if case == "home_mode" {
            fs::set_permissions(fixture.home(), fs::Permissions::from_mode(0o755)).unwrap();
        } else {
            fs::rename(fixture.home(), fixture.0.path().join("elsewhere")).unwrap();
            symlink(fixture.0.path().join("elsewhere"), fixture.home()).unwrap();
        }
        let home = match case {
            "home_link_slash" => format!("{}/", fixture.home().display()),
            "home_link_dot" => format!("{}/.", fixture.home().display()),
            _ => fixture.home().display().to_string(),
        };
        let output = fixture
            .command()
            .env("PONY_HOME", &home)
            .arg("run")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{case}: {output:?}");
        let run = decode(&output);
        assert_eq!(run["error"], "secrets_too_open", "{case}: {run}");
        assert!(!fixture.home().join("run").exists());
        let output = fixture
            .command()
            .env("PONY_HOME", &home)
            .arg("status")
            .output()
            .unwrap();
        assert!(output.status.success(), "{case}: {output:?}");
        let status = decode(&output);
        assert_eq!(status["config"], "invalid", "{case}: {status}");
        fixture.assert_private(&[run, status]);
    }
}

#[test]
fn missing_or_invalid_secrets_are_reported_from_run_without_echoing_toml() {
    let fixture = Fixture::configured();
    let path = fixture.home().join("secrets/slack.toml");
    for (source, expected) in [
        (None, "secrets_missing"),
        (
            Some("app_token = 'xapp-test'\nbot_token = 'xoxb-test"),
            "secrets_invalid",
        ),
        (
            Some("app_token = 'xapp-test'\nbot_token = ''"),
            "secrets_invalid",
        ),
    ] {
        if let Some(source) = source {
            fs::write(&path, source).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        } else {
            fs::remove_file(&path).unwrap();
        }
        let run = fixture.call(&["run"], 1);
        assert_eq!(run["error"], expected, "{run}");
        let status = fixture.call(&["status"], 0);
        assert_eq!(status["config"], "invalid");
        assert_eq!(status["last_exit"]["code"], expected);
        assert!(status["last_exit"]["at"].is_number());
        fixture.assert_private(&[run, status]);
    }
}

struct Running(Option<Child>);

impl Running {
    fn start(fixture: &Fixture) -> Self {
        Self(Some(
            fixture
                .command()
                .arg("run")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        ))
    }

    fn wait_for(&mut self, fixture: &Fixture, predicate: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            assert!(
                self.0.as_mut().unwrap().try_wait().unwrap().is_none(),
                "pony run exited before the expected state"
            );
            let status = fixture.call(&["status"], 0);
            if predicate(&status) {
                return status;
            }
            assert!(Instant::now() < deadline, "timed out: {status}");
            thread::sleep(Duration::from_millis(40));
        }
    }

    fn stop(mut self, signal: i32) -> Output {
        let child = self.0.take().unwrap();
        assert_eq!(unsafe { libc::kill(child.id() as i32, signal) }, 0);
        child.wait_with_output().unwrap()
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[test]
fn running_holds_one_lock_reloads_config_and_stale_status_is_off() {
    let fixture = Fixture::configured();
    let mut running = Running::start(&fixture);
    let status = running.wait_for(&fixture, |s| {
        s["running"] == true
            && s["chats"][0]["connection"] == "failed"
            && s["chats"][0]["since"].is_number()
    });
    assert_eq!(status["pid"], running.0.as_ref().unwrap().id());
    assert!(status["started"].as_f64().unwrap() > 0.0);
    assert_eq!(status["config"], "ok");
    assert!(status["last_exit"].is_null());
    assert_eq!(status["chats"][0]["reason"], "network");
    assert_eq!(status["chats"][0]["message"], "Slack: network");
    for key in ["workspace", "bot"] {
        assert!(status["chats"][0][key].is_null());
    }
    let snapshot = fs::read(fixture.home().join("run/status.json")).unwrap();
    assert_eq!(fixture.call(&["run"], 1)["error"], "already_running");
    assert_eq!(
        fs::read(fixture.home().join("run/status.json")).unwrap(),
        snapshot
    );
    fixture.assert_private(&[status]);

    fs::write(fixture.home().join("config.toml"), "[slack\nprivate-owner").unwrap();
    thread::sleep(Duration::from_millis(2300));
    let invalid = running.wait_for(&fixture, |s| {
        s["config"] == "invalid" && s["chats"][0]["connection"] == "failed"
    });
    assert_eq!(invalid["chats"][0]["channels"][0]["repo"], "ranch");
    assert_eq!(invalid["chats"][0]["connection"], "failed");
    fixture.write_config(&CONFIG.replace("'ranch'", "'other'"));
    let updated = running.wait_for(&fixture, |s| {
        s["config"] == "ok" && s["chats"][0]["channels"][0]["repo"] == "other"
    });
    assert!(updated["config_message"].is_null());
    let output = running.stop(libc::SIGTERM);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(decode(&output)["ok"], true);
    let stopped = fixture.call(&["status"], 0);
    assert_eq!(stopped["running"], false);
    assert!(stopped["pid"].is_null());
    assert!(stopped["started"].is_null());
    assert_eq!(stopped["chats"][0]["connection"], "off");
    assert_eq!(stopped["chats"][0]["since"], stopped["last_exit"]["at"]);
    assert_eq!(stopped["last_exit"]["code"], "stopped");
    fixture.assert_private(&[invalid, updated, stopped]);

    let mut restarted = Running::start(&fixture);
    let restarted_pid = restarted.0.as_ref().unwrap().id();
    restarted.wait_for(&fixture, |s| {
        s["running"] == true
            && s["chats"][0]["connection"] == "failed"
            && fs::read(fixture.home().join("run/status.json"))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                .is_some_and(|snapshot| snapshot["pid"] == restarted_pid)
    });
    restarted.stop(libc::SIGKILL);
    let crashed = fixture.call(&["status"], 0);
    assert_eq!(crashed["running"], false);
    assert_eq!(crashed["chats"][0]["connection"], "off");
    assert!(crashed["chats"][0]["since"].is_null());
    assert!(crashed["last_exit"].is_null());
}

#[test]
fn home_defaults_and_missing_environment_do_not_use_the_working_directory() {
    let fixture = Fixture::new();
    fs::write(fixture.0.path().join("config.toml"), "invalid TOML").unwrap();
    for pony_home in [None, Some("")] {
        let mut command = fixture.command();
        command.env_remove("PONY_HOME");
        if let Some(home) = pony_home {
            command.env("PONY_HOME", home);
        }
        let output = command.arg("status").output().unwrap();
        assert!(output.status.success(), "{output:?}");
        let status = decode(&output);
        assert_eq!(status["config"], "missing");
        assert!(
            status["config_message"]
                .as_str()
                .unwrap()
                .contains("/.pony/config.toml")
        );
    }
    for action in ["status", "run"] {
        let output = fixture
            .command()
            .env("PONY_HOME", "")
            .env_remove("HOME")
            .arg(action)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let error = decode(&output);
        assert_eq!(error["error"], "invalid_data");
        assert_eq!(error["message"], "HOME is not set");
    }
    assert!(!fixture.0.path().join(".pony").exists());
    assert!(!fixture.home().exists());
}

#[test]
fn unknown_keys_report_safe_names_or_source_locations_without_values() {
    let fixture = Fixture::configured();
    for (source, position) in [
        ("unknown = 'xoxb-test'\n".to_owned(), "unknown"),
        (
            format!("{CONFIG}bot_token = 'xoxb-test'\n"),
            "slack.bot_token",
        ),
        (
            CONFIG.replace("repo = 'ranch'", "repo = 'ranch', rpo = 'xoxb-test'"),
            "slack.channels[0].rpo",
        ),
        (
            "# comment\n'xoxb-test' = 'xapp-test'\n".to_owned(),
            "config.toml:2:",
        ),
        (
            format!("{CONFIG}'xoxb-test' = 'xapp-test'\n"),
            "config.toml:4:",
        ),
        (
            CONFIG.replace(
                "repo = 'ranch'",
                "repo = 'ranch', 'xoxb-test' = 'xapp-test'",
            ),
            "config.toml:3:",
        ),
        (
            "# comment\n'escaped\\key' = 'xapp-test'\n".to_owned(),
            "config.toml:2:",
        ),
    ] {
        fs::write(fixture.home().join("config.toml"), source).unwrap();
        let status = fixture.call(&["status"], 0);
        assert_eq!(status["config"], "invalid", "{status}");
        let message = status["config_message"].as_str().unwrap();
        assert!(message.contains(position), "{message}");
        assert!(message.contains("unknown key"), "{message}");
        fixture.assert_private(&[status]);
    }
}

#[test]
fn run_checks_corral_contract_before_starting_the_core() {
    let fixture = Fixture::configured();
    let response = fixture.0.path().join("response.json");
    fs::write(&response, r#"{"ok":true,"contract":"unsupported"}"#).unwrap();
    let mut child = fixture
        .command()
        .env("FAKE_CORRAL_RESPONSE", &response)
        .arg("run")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let exited = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            break None;
        }
        thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(exited.and_then(|s| s.code()), Some(1));
    let output = child.wait_with_output().unwrap();
    assert_eq!(decode(&output)["error"], "corral_protocol");
}
