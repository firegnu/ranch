//! Real srt only: FENCE_TEST_SRT points at an npm prefix containing srt 0.0.80.
use serde_json::Value;
use std::{
    fs,
    os::unix::process::{CommandExt, ExitStatusExt},
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

struct Lab {
    _temp: tempfile::TempDir,
    root: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
}
impl Lab {
    fn new() -> Self {
        let srt = PathBuf::from(
            std::env::var_os("FENCE_TEST_SRT").expect("set FENCE_TEST_SRT to npm prefix"),
        );
        // macOS Unix sockets have a short sun_path; its default temporary directory is too long.
        let temp = tempfile::tempdir_in("/tmp").unwrap();
        let root = temp.path().canonicalize().unwrap();
        let home = root.join("home");
        let cwd = home.join("worktree");
        for dir in [
            root.join("bin"),
            root.join("share/fence"),
            root.join("lib"),
            cwd.clone(),
        ] {
            fs::create_dir_all(dir).unwrap();
        }
        fs::copy(env!("CARGO_BIN_EXE_fence"), root.join("bin/fence")).unwrap();
        fs::copy(
            concat!(env!("CARGO_MANIFEST_DIR"), "/resources/runner.mjs"),
            root.join("share/fence/runner.mjs"),
        )
        .unwrap();
        std::os::unix::fs::symlink(&srt, root.join("lib/srt")).unwrap();
        let node = Command::new("node")
            .args(["-p", "process.execPath"])
            .output()
            .unwrap();
        assert!(node.status.success());
        let node = String::from_utf8(node.stdout).unwrap().trim().to_owned();
        fs::write(
            root.join("share/fence/runtime.toml"),
            format!(
                "node = {}\nnode_version = 'test'\nsrt_version = '0.0.80'\n",
                serde_json::to_string(&node).unwrap()
            ),
        )
        .unwrap();
        Self {
            _temp: temp,
            root,
            home,
            cwd,
        }
    }
    fn command(&self) -> Command {
        let mut c = Command::new(self.root.join("bin/fence"));
        c.env("HOME", &self.home)
            .env("FENCE_HOME", self.home.join(".fence"))
            .env_remove("XDG_CACHE_HOME")
            .env_remove("FENCE_RUNNER")
            .env_remove("CARGO_TARGET_DIR")
            .current_dir(&self.cwd);
        c
    }
    fn run(&self, id: &str, script: &str) -> Command {
        let mut c = self.command();
        c.args([
            "run",
            "--profile",
            "dev",
            "--kind",
            "claude",
            "--agent",
            "test/srt",
            "--instance",
            id,
            "--",
            "/bin/sh",
            "-c",
            script,
        ]);
        c
    }
}

#[test]
#[ignore = "requires local srt 0.0.80 and its OS sandbox dependencies"]
fn real_srt_enforces_files_network_and_reports_denials() {
    let l = Lab::new();
    fs::write(l.home.join("fake-key"), "not-a-real-secret").unwrap();
    fs::create_dir(l.home.join("main-repo")).unwrap();
    fs::write(l.home.join("main-repo/protected"), "unchanged").unwrap();
    let script = r#"
set -eu
printf allowed > "$TMPDIR/allowed"
printf allowed > allowed
if cat "$HOME/fake-key"; then exit 41; fi
if printf changed > "$HOME/main-repo/protected"; then exit 42; fi
if printf changed > "$FENCE_HOME/local.toml"; then exit 43; fi
if printf changed > .fence.toml; then exit 44; fi
/usr/bin/curl -sS --max-time 10 -o response -w '%{http_code}' http://example.com > http-code
test "$(cat http-code)" = 403
"#;
    let out = l.run("srt-test", script).output().unwrap();
    assert!(
        out.status.success(),
        "status {:?}\nstdout {}\nstderr {}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        fs::read_to_string(l.home.join("main-repo/protected")).unwrap(),
        "unchanged"
    );
    let out = l
        .command()
        .args(["denials", "--agent", "test/srt"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], true, "{v}");
    assert!(
        v["denials"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["type"] == "network" && d["target"] == "example.com:80"),
        "{v}"
    );
    assert_eq!(v["running"], false);
    eprintln!("srt monitors: {}", v["monitors"]);
}

#[test]
#[ignore = "requires local srt 0.0.80 and its OS sandbox dependencies"]
fn real_srt_runner_preserves_normal_exit_and_group_hup_status() {
    let l = Lab::new();
    assert_eq!(l.run("normal", "exit 9").status().unwrap().code(), Some(9));
    for (id, script, expected_code, expected_signal) in [
        (
            "handled",
            "trap 'exit 23' HUP; echo ready > ready; while :; do sleep 1; done",
            Some(23),
            None,
        ),
        (
            "signal",
            "echo ready > ready; exec /bin/sleep 60",
            None,
            Some(libc::SIGHUP),
        ),
    ] {
        let _ = fs::remove_file(l.cwd.join("ready"));
        let mut c = l.run(id, script);
        c.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        unsafe {
            c.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = c.spawn().unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        while !l.cwd.join("ready").exists() && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(10));
        }
        let ready = l.cwd.join("ready").exists();
        unsafe {
            libc::kill(-(child.id() as i32), libc::SIGHUP);
        }
        let mut status = None;
        while Instant::now() < end {
            status = child.try_wait().unwrap();
            if status.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        if status.is_none() {
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
        }
        let _ = child.wait();
        assert!(ready, "agent never became ready");
        let status = status.expect("runner did not finish after group HUP");
        assert_eq!(status.code(), expected_code, "{id}");
        assert_eq!(status.signal(), expected_signal, "{id}");
    }
}

#[test]
#[ignore = "requires local srt 0.0.80 and its OS sandbox dependencies"]
fn packed_slash_branch_can_commit_and_amend_without_writing_other_refs() {
    let l = Lab::new();
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .env("HOME", &l.home)
            .current_dir(&l.cwd)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    git(&["init", "-b", "main"]);
    git(&["config", "user.name", "Fence Test"]);
    git(&["config", "user.email", "fence@example.invalid"]);
    git(&["commit", "--allow-empty", "-m", "initial"]);
    let worktree = l.home.join("linked");
    git(&[
        "worktree",
        "add",
        "-b",
        "feature/x",
        worktree.to_str().unwrap(),
    ]);
    git(&["pack-refs", "--all", "--prune"]);
    assert!(!l.cwd.join(".git/refs/heads/feature").exists());
    let out = l.run("packed", "set -e; echo changed > change; git add change; git commit -m changed; git commit --amend --no-edit")
        .current_dir(&worktree).output().unwrap();
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
