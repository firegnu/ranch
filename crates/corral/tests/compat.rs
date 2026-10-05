//! Explicit, external baseline comparison. Not needed to build or run the Rust product.
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{Duration, Instant},
};
struct Lab {
    root: tempfile::TempDir,
    old: PathBuf,
}
impl Lab {
    fn output(&self, bin: &Path, args: &[&str]) -> Output {
        Command::new(bin)
            .args(args)
            .env("CORRAL_HOME", self.root.path().join("pens"))
            .env("HOME", self.root.path())
            .env("SHELL", "/bin/sh")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .env_remove("CODEX_SANDBOX")
            .output()
            .unwrap()
    }
    fn json(&self, bin: &Path, args: &[&str]) -> Value {
        let o = self.output(bin, args);
        assert!(
            o.status.success(),
            "{bin:?} {args:?}: {} {}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        serde_json::from_slice(&o.stdout).unwrap()
    }
}
impl Drop for Lab {
    fn drop(&mut self) {
        for name in ["mix/old", "mix/new"] {
            for bin in [&*self.old, Path::new(env!("CARGO_BIN_EXE_corral"))] {
                let _ = Command::new(bin)
                    .args(["stop", name, "--timeout", "4"])
                    .env("CORRAL_HOME", self.root.path().join("pens"))
                    .env("HOME", self.root.path())
                    .env("PYTHONDONTWRITEBYTECODE", "1")
                    .env_remove("CODEX_SANDBOX")
                    .output();
            }
        }
    }
}
#[test]
#[ignore = "set CORRAL_COMPAT_BIN to an unchanged external contract-v1 baseline"]
fn both_clients_operate_both_pens_in_the_same_namespace() {
    let lab = Lab {
        root: tempfile::tempdir_in("/tmp").unwrap(),
        old: std::env::var_os("CORRAL_COMPAT_BIN")
            .expect("explicit baseline required")
            .into(),
    };
    let new = Path::new(env!("CARGO_BIN_EXE_corral"));
    for (starter, name) in [(&*lab.old, "mix/old"), (new, "mix/new")] {
        let started = lab.json(
            starter,
            &[
                "start",
                name,
                "--cwd",
                "/tmp",
                "--label",
                "role=regular",
                "--",
                "/bin/cat",
            ],
        );
        for client in [&*lab.old, new] {
            let st = lab.json(client, &["status", name]);
            assert_eq!(st["instance"], started["instance"]);
            assert_eq!(st["state"], "unknown");
            assert_eq!(st["labels"]["role"], "regular");
            assert_eq!(lab.json(client, &["where", name])["cwd"], "/tmp");
            lab.json(client, &["keys", name, "text:matrix", "enter"]);
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let o = lab.output(client, &["read", name]);
                assert!(o.status.success());
                if String::from_utf8_lossy(&o.stdout).contains("matrix") {
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
    for client in [&*lab.old, new] {
        let list = lab.json(client, &["ls"]);
        assert_eq!(list["agents"].as_array().unwrap().len(), 2);
    }
    lab.json(new, &["stop", "mix/old", "--timeout", "4"]);
    lab.json(&lab.old, &["stop", "mix/new", "--timeout", "4"]);
    for client in [&*lab.old, new] {
        assert_eq!(lab.json(client, &["ls"])["agents"], serde_json::json!([]));
    }
}

#[test]
#[ignore = "set CORRAL_COMPAT_BIN to an unchanged external contract-v1 baseline"]
fn alternating_clients_share_v1_events_cursor_and_confirmed_sends() {
    use serde_json::json;
    use std::{fs, io::Write, os::unix::fs::PermissionsExt};
    let lab = Lab {
        root: tempfile::tempdir_in("/tmp").unwrap(),
        old: std::env::var_os("CORRAL_COMPAT_BIN")
            .expect("explicit baseline required")
            .into(),
    };
    let staged = lab.root.path().join("corral");
    fs::copy(env!("CARGO_BIN_EXE_corral"), &staged).unwrap();
    let new = staged.as_path();
    let fake = lab.root.path().join("claude");
    fs::write(&fake, "#!/bin/sh\nexec /bin/cat\n").unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();
    for (starter, name) in [(&*lab.old, "mix/old"), (new, "mix/new")] {
        let s = lab.json(
            starter,
            &["start", name, "--cwd", "/tmp", "--", fake.to_str().unwrap()],
        );
        let event_file = lab.root.path().join("pens").join(name).join("events");
        let emit = |ev: &str, extra: Value| {
            let mut v = json!({"v":1,"inst":s["instance"],"ev":ev,"session_id":"main","t":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64()});
            v.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            let mut f = fs::OpenOptions::new()
                .append(true)
                .open(&event_file)
                .unwrap();
            writeln!(f, "{v}").unwrap();
        };
        emit("UserPromptSubmit", json!({"prompt":"before start"}));
        emit("SessionStart", json!({"cwd":"/tmp","has_transcript":true}));
        for client in [new, &*lab.old, new] {
            assert_eq!(lab.json(client, &["status", name])["state"], "working");
        }
        let text = "full reply 中文\r\n```\nblock\n```\u{0}";
        emit(
            "Stop",
            json!({"last_assistant_message":text,"background_running":0}),
        );
        for client in [&*lab.old, new, &*lab.old] {
            let r = lab.json(client, &["reply", name]);
            assert_eq!(r["text"], text);
            assert!(r["at"].is_number());
        }
        // A partial UTF-8 event is not consumed; the other client may complete it later.
        let partial=json!({"v":1,"inst":s["instance"],"ev":"PreToolUse","session_id":"main","t":1.5,"tool_name":"工具"}).to_string();
        let split = partial.len() - 3;
        let mut f = fs::OpenOptions::new()
            .append(true)
            .open(&event_file)
            .unwrap();
        f.write_all(&partial.as_bytes()[..split]).unwrap();
        assert_eq!(lab.json(new, &["status", name])["state"], "idle");
        f.write_all(&partial.as_bytes()[split..]).unwrap();
        f.write_all(b"\n").unwrap();
        drop(f);
        assert_eq!(lab.json(&lab.old, &["status", name])["last_tool"], "工具");
        assert_eq!(lab.json(new, &["status", name])["last_tool"], "工具");
        for (i, client) in [new, &*lab.old].into_iter().enumerate() {
            emit("Stop", json!({"last_assistant_message":"ready"}));
            let text = format!("message-{i}");
            let sent = std::thread::scope(|scope| {
                let borrowed = &text;
                let worker =
                    scope.spawn(|| lab.json(client, &["send", name, borrowed, "--timeout", "3"]));
                let deadline = Instant::now() + Duration::from_secs(3);
                loop {
                    let out = lab.output(new, &["read", name]);
                    if String::from_utf8_lossy(&out.stdout).contains(&text) {
                        break;
                    }
                    assert!(Instant::now() < deadline);
                    std::thread::sleep(Duration::from_millis(20));
                }
                emit(
                    "UserPromptSubmit",
                    json!({"prompt":format!("draft {text}")}),
                );
                worker.join().unwrap()
            });
            assert_eq!(sent["confirmed"], true);
            assert_eq!(sent["merged_with_draft"], true);
        }
    }
}

#[test]
#[ignore = "set CORRAL_COMPAT_BIN to an unchanged external contract-v1 baseline"]
fn both_attach_clients_resize_and_detach_from_both_pens() {
    use std::{
        fs,
        io::{Read, Write},
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::process::CommandExt,
        },
    };
    struct Attached {
        child: std::process::Child,
        master: fs::File,
        bytes: Vec<u8>,
    }
    impl Drop for Attached {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
    impl Attached {
        fn see(&mut self, text: &str) {
            let until = Instant::now() + Duration::from_secs(5);
            let mut buf = [0; 65536];
            loop {
                if let Ok(n) = self.master.read(&mut buf) {
                    self.bytes.extend_from_slice(&buf[..n]);
                }
                if String::from_utf8_lossy(&self.bytes).contains(text) {
                    return;
                }
                assert!(
                    Instant::now() < until,
                    "missing {text}: {}",
                    String::from_utf8_lossy(&self.bytes)
                );
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
    let lab = Lab {
        root: tempfile::tempdir_in("/tmp").unwrap(),
        old: std::env::var_os("CORRAL_COMPAT_BIN")
            .expect("explicit baseline required")
            .into(),
    };
    let new = lab.root.path().join("corral");
    fs::copy(env!("CARGO_BIN_EXE_corral"), &new).unwrap();
    for (starter, name) in [(&*lab.old, "mix/old"), (new.as_path(), "mix/new")] {
        let started = lab.json(
            starter,
            &[
                "start",
                name,
                "--",
                "/bin/sh",
                "-c",
                "while IFS= read -r line; do printf 'MARK:%s:' \"$line\"; stty size; done",
            ],
        );
        for client in [&*lab.old, new.as_path()] {
            let (mut m, mut s) = (-1, -1);
            let mut size = libc::winsize {
                ws_row: 24,
                ws_col: 80,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            assert_eq!(
                unsafe {
                    libc::openpty(
                        &mut m,
                        &mut s,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        &mut size,
                    )
                },
                0
            );
            let master = unsafe { fs::File::from_raw_fd(m) };
            let slave = unsafe { fs::File::from_raw_fd(s) };
            unsafe {
                libc::fcntl(m, libc::F_SETFD, libc::FD_CLOEXEC);
                libc::fcntl(m, libc::F_SETFL, libc::O_NONBLOCK);
            }
            let mut cmd = Command::new(client);
            cmd.args(["attach", name])
                .env("HOME", lab.root.path())
                .env("CORRAL_HOME", lab.root.path().join("pens"))
                .env("PYTHONDONTWRITEBYTECODE", "1")
                .env_remove("CODEX_SANDBOX")
                .stdin(slave.try_clone().unwrap())
                .stdout(slave.try_clone().unwrap())
                .stderr(slave.try_clone().unwrap());
            unsafe {
                cmd.pre_exec(|| {
                    libc::setsid();
                    if libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            let child = cmd.spawn().unwrap();
            drop(cmd);
            drop(slave);
            let mut a = Attached {
                child,
                master,
                bytes: Vec::new(),
            };
            let until = Instant::now() + Duration::from_secs(5);
            while lab.json(&new, &["status", name])["attached"] != 1 {
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(20));
            }
            // Allow the pen's v1 delayed resize repair to settle before asking the PTY.
            std::thread::sleep(Duration::from_millis(200));
            a.master.write_all(b"initial\r").unwrap();
            a.see("MARK:initial:24 80");
            size.ws_row = 35;
            size.ws_col = 100;
            assert_eq!(
                unsafe { libc::ioctl(a.master.as_raw_fd(), libc::TIOCSWINSZ as _, &size) },
                0
            );
            unsafe {
                libc::kill(a.child.id() as i32, libc::SIGWINCH);
            }
            std::thread::sleep(Duration::from_millis(300));
            a.master.write_all(b"resized\r").unwrap();
            a.see("MARK:resized:35 100");
            a.master.write_all(&[0x1d]).unwrap();
            let until = Instant::now() + Duration::from_secs(3);
            loop {
                let mut bytes = [0; 4096];
                if let Ok(n) = a.master.read(&mut bytes) {
                    a.bytes.extend_from_slice(&bytes[..n]);
                }
                if let Some(status) = a.child.try_wait().unwrap() {
                    assert!(status.success());
                    break;
                }
                assert!(
                    Instant::now() < until,
                    "{client:?} did not detach from {name}: {}",
                    String::from_utf8_lossy(&a.bytes)
                );
                std::thread::sleep(Duration::from_millis(20));
            }
            assert_eq!(
                lab.json(&new, &["status", name])["instance"],
                started["instance"]
            );
        }
    }
}
