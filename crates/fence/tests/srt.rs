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
fn symlinked_policy_and_agent_config_targets_remain_write_protected() {
    let l = Lab::new();
    fs::write(l.cwd.join("repo-policy"), "domains = []\n").unwrap();
    fs::create_dir(l.cwd.join("policy-home")).unwrap();
    fs::write(l.cwd.join("policy-home/local.toml"), "domains = []\n").unwrap();
    fs::create_dir(l.home.join(".claude")).unwrap();
    fs::write(l.cwd.join("agent-settings"), "unchanged").unwrap();
    for (target, link) in [
        (l.cwd.join("repo-policy"), l.cwd.join(".fence.toml")),
        (l.cwd.join("policy-home"), l.home.join(".fence")),
        (
            l.cwd.join("agent-settings"),
            l.home.join(".claude/settings.json"),
        ),
    ] {
        std::os::unix::fs::symlink(target, link).unwrap();
    }
    let out = l.run("symlink-policy", r#"
printf allowed > ordinary
for path in policy-home/local.toml repo-policy .fence.toml "$FENCE_HOME/local.toml" agent-settings "$HOME/.claude/settings.json"; do
  if printf bad > "$path"; then echo "writable: $path"; fi
done
if rm .fence.toml; then echo 'removed policy link'; fi
"#).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(
        fs::read_to_string(l.cwd.join("ordinary")).unwrap(),
        "allowed"
    );
    assert_eq!(
        fs::read_to_string(l.cwd.join("repo-policy")).unwrap(),
        "domains = []\n"
    );
    assert_eq!(
        fs::read_to_string(l.cwd.join("policy-home/local.toml")).unwrap(),
        "domains = []\n"
    );
    assert_eq!(
        fs::read_to_string(l.cwd.join("agent-settings")).unwrap(),
        "unchanged"
    );
    assert!(l.cwd.join(".fence.toml").is_symlink());
}

#[test]
#[ignore = "requires local srt 0.0.80 and its OS sandbox dependencies"]
fn intermediate_policy_links_cannot_be_replaced() {
    let l = Lab::new();
    fs::write(l.cwd.join("policy-old"), "domains = []\n").unwrap();
    fs::write(l.cwd.join("policy-new"), "domains = ['example.com']\n").unwrap();
    for (target, link) in [("policy-old", "policy-hop"), ("policy-hop", ".fence.toml")] {
        std::os::unix::fs::symlink(target, l.cwd.join(link)).unwrap();
    }
    for dir in ["home-old", "home-new", "settings-old", "settings-new"] {
        fs::create_dir(l.cwd.join(dir)).unwrap();
    }
    fs::write(l.cwd.join("home-old/local.toml"), "domains = []\n").unwrap();
    fs::write(l.cwd.join("settings-old/config.json"), "{}").unwrap();
    fs::write(l.cwd.join("settings-new/config.json"), "{\"changed\":true}").unwrap();
    fs::create_dir(l.home.join(".claude")).unwrap();
    for (target, link) in [
        (l.cwd.join("home-hop"), l.home.join(".fence")),
        (PathBuf::from("home-old"), l.cwd.join("home-hop")),
        (
            l.cwd.join("settings-parent/config.json"),
            l.home.join(".claude/settings.json"),
        ),
        (PathBuf::from("settings-hop"), l.cwd.join("settings-parent")),
        (PathBuf::from("settings-old"), l.cwd.join("settings-hop")),
    ] {
        std::os::unix::fs::symlink(target, link).unwrap();
    }
    let out = l
        .run(
            "policy-chain",
            r#"
printf allowed > ordinary
ln -sf policy-new policy-hop
echo retarget:$?
cat .fence.toml
rm policy-hop
echo remove:$?
mv policy-hop moved-policy
echo rename:$?
ln -sfn home-new home-hop
echo home:$?
ln -sfn settings-new settings-hop
echo settings:$?
"#,
        )
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "retarget:1\ndomains = []\nremove:1\nrename:1\nhome:1\nsettings:1\n"
    );
    assert_eq!(
        fs::read_link(l.cwd.join("policy-hop")).unwrap(),
        PathBuf::from("policy-old")
    );
    assert_eq!(
        fs::read_link(l.cwd.join(".fence.toml")).unwrap(),
        PathBuf::from("policy-hop")
    );
    assert_eq!(
        fs::read_link(l.cwd.join("home-hop")).unwrap(),
        PathBuf::from("home-old")
    );
    assert_eq!(
        fs::read_link(l.cwd.join("settings-parent")).unwrap(),
        PathBuf::from("settings-hop")
    );
    assert_eq!(
        fs::read_link(l.cwd.join("settings-hop")).unwrap(),
        PathBuf::from("settings-old")
    );
    assert!(!l.cwd.join("moved-policy").exists());
    assert_eq!(
        fs::read_to_string(l.cwd.join("ordinary")).unwrap(),
        "allowed"
    );
}

#[test]
#[ignore = "requires local srt 0.0.80 and its OS sandbox dependencies"]
fn domain_preflight_agrees_with_the_locked_srt_schema() {
    let l = Lab::new();
    let patterns = [
        "example.com",
        "*.example.com",
        "*.nested.example.com:8443",
        "localhost:1",
        "127.0.0.1:65535",
        "[::1]",
        "[2001:db8::1]:443",
        "[::ffff:192.0.2.1]:80",
        "[fe80::1%eth0]:443",
        "example..com",
        "*.foo*.example.com",
        "https://example.com",
        "example.com/path",
        "*",
        "*.com",
        "*example.com",
        "example.com:0",
        "example.com:65536",
        "example.com:0443",
        "example.com:https",
        "[::1]:0",
        "[::1]:65536",
        "::1",
        "[invalid::ip]",
    ];
    let out = Command::new("node")
        .env(
            "SRT_MODULE",
            l.root
                .join("lib/srt/node_modules/@anthropic-ai/sandbox-runtime/dist/index.js"),
        )
        .args([
            "--input-type=module",
            "-e",
            r#"
const { SandboxRuntimeConfigSchema } = await import(process.env.SRT_MODULE);
console.log(JSON.stringify(JSON.parse(process.argv[1]).map(domain =>
  SandboxRuntimeConfigSchema.safeParse({
    network: { allowedDomains: [domain], deniedDomains: [] },
    filesystem: { denyRead: [], allowWrite: [], denyWrite: [] }
  }).success)));
"#,
        ])
        .arg(serde_json::to_string(&patterns).unwrap())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let accepted: Vec<bool> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(accepted.len(), patterns.len());
    fs::create_dir(l.home.join(".fence")).unwrap();
    for (domain, expected) in patterns.into_iter().zip(accepted) {
        fs::write(
            l.home.join(".fence/local.toml"),
            format!("domains = [{}]\n", serde_json::json!(domain)),
        )
        .unwrap();
        let out = l
            .command()
            .args(["check", "--profile", "dev"])
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["ok"], expected, "{domain}: {v}");
    }
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
