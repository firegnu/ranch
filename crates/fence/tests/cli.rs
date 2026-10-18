use serde_json::{Value, json};
use std::{fs, process::Command};

struct Fixture(tempfile::TempDir);
impl Fixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("fence")).unwrap();
        Self(tmp)
    }
    fn command(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_fence"));
        cmd.env_clear()
            .env("HOME", self.0.path())
            .env("FENCE_HOME", self.0.path().join("fence"))
            .env("FENCE_RUNNER", "/bin/sh")
            .env("PATH", "/usr/bin:/bin")
            .current_dir(self.0.path());
        cmd
    }
    fn call(&self, args: &[&str]) -> Value {
        let out = self.command().args(args).output().unwrap();
        let value: Value =
            serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{e}: {:?}", out));
        assert_eq!(out.status.success(), value["ok"] == true, "{value}");
        value
    }
}

#[test]
fn deny_write_resolves_linked_parents_and_rejects_unresolvable_targets() {
    let f = Fixture::new();
    let root = f.0.path();
    fs::create_dir(root.join("actual")).unwrap();
    let actual = root.join("actual").canonicalize().unwrap();
    std::os::unix::fs::symlink(root.join("actual"), root.join("alias")).unwrap();
    fs::write(
        root.join("fence/local.toml"),
        "deny_write = ['${home}/alias/missing/config']\n",
    )
    .unwrap();
    let v = f.call(&["check", "--profile", "dev"]);
    assert_eq!(v["ok"], true, "{v}");
    let denies = v["settings"]["filesystem"]["denyWrite"].as_array().unwrap();
    assert!(denies.contains(&json!(root.join("alias/missing/config"))));
    assert!(denies.contains(&json!(actual.join("missing/config"))));
    // FENCE_HOME can itself contain a linked parent and not exist yet.
    let out = f
        .command()
        .env("FENCE_HOME", root.join("alias/new/fence"))
        .args(["check", "--profile", "dev"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], true, "{v}");
    assert!(
        v["settings"]["filesystem"]["denyWrite"]
            .as_array()
            .unwrap()
            .contains(&json!(actual.join("new/fence")))
    );
    for target in ["absent", "alias"] {
        fs::remove_file(root.join("alias")).unwrap();
        std::os::unix::fs::symlink(target, root.join("alias")).unwrap();
        let v = f.call(&["check", "--profile", "dev"]);
        assert_eq!(v["error"], "invalid_data", "{target}: {v}");
        assert_eq!(v["problems"][0]["path"], "deny_write");
        let out = f
            .command()
            .args([
                "run",
                "--profile",
                "dev",
                "--kind",
                "claude",
                "--agent",
                "test/policy",
                "--instance",
                "broken",
                "--",
                "/bin/sh",
                "-c",
                "touch launched",
            ])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(125));
        assert!(!root.join("launched").exists());
        assert!(!root.join("fence/runs").exists());
    }
}

#[test]
fn domain_patterns_are_checked_before_start_with_source_and_field() {
    let f = Fixture::new();
    let policy = f.0.path().join(".fence.toml");
    fs::create_dir(f.0.path().join("bin")).unwrap();
    std::os::unix::fs::symlink("/bin/sh", f.0.path().join("bin/claude")).unwrap();
    for domain in [
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
    ] {
        fs::write(&policy, format!("domains = [{}]\n", json!(domain))).unwrap();
        let out = f
            .command()
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", f.0.path().join("bin").display()),
            )
            .args(["check", "--profile", "dev", "--kind", "claude"])
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["error"], "invalid_data", "{domain}: {v}");
        assert_eq!(
            v["problems"][0]["source"],
            json!(policy.canonicalize().unwrap())
        );
        assert_eq!(v["problems"][0]["path"], "domains[0]");
        let out = f
            .command()
            .args([
                "run",
                "--profile",
                "dev",
                "--kind",
                "claude",
                "--agent",
                "test/domain",
                "--instance",
                "invalid",
                "--",
                "/bin/sh",
                "-c",
                "touch launched",
            ])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(125));
        let v: Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(v["error"], "invalid_data", "{domain}: {v}");
        assert_eq!(v["problems"][0]["path"], "domains[0]");
        assert!(!f.0.path().join("launched").exists());
        assert!(!f.0.path().join("fence/runs").exists());
    }
    let valid = json!([
        "example.com",
        "*.example.com",
        "*.nested.example.com:8443",
        "localhost:1",
        "127.0.0.1:65535",
        "[::1]",
        "[2001:db8::1]:443",
        "[::ffff:192.0.2.1]:80",
        "[fe80::1%eth0]:443"
    ]);
    fs::write(
        f.0.path().join("fence/local.toml"),
        format!("domains = {valid}\n"),
    )
    .unwrap();
    let v = f.call(&["check", "--profile", "dev"]);
    assert_eq!(v["ok"], true, "{v}");
    for domain in valid.as_array().unwrap() {
        assert!(
            v["settings"]["network"]["allowedDomains"]
                .as_array()
                .unwrap()
                .contains(domain)
        );
    }
}

#[test]
fn profiles_list_builtin_and_user_replacements_even_when_invalid() {
    let f = Fixture::new();
    let result = f.call(&["profiles"]);
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["profiles"][0]["name"], "dev");
    assert_eq!(result["profiles"][0]["source"], "builtin");
    assert_eq!(result["kinds"].as_array().unwrap().len(), 4);
    fs::create_dir(f.0.path().join("fence/profiles")).unwrap();
    fs::write(f.0.path().join("fence/profiles/dev.toml"), "broken = [").unwrap();
    let result = f.call(&["profiles"]);
    assert_eq!(result["ok"], true);
    assert_eq!(result["profiles"][0]["source"], "user");
    assert_eq!(result["profiles"][0]["overrides_builtin"], true);
    assert!(result["profiles"][0]["error"].is_object());
    assert_eq!(f.call(&["--version"])["contract"], json!("1"));
}

#[test]
fn check_merges_rules_and_rejects_forbidden_repository_keys() {
    let f = Fixture::new();
    fs::create_dir(f.0.path().join("fence/agents")).unwrap();
    fs::create_dir(f.0.path().join("bin")).unwrap();
    std::os::unix::fs::symlink("/bin/sh", f.0.path().join("bin/new-agent")).unwrap();
    fs::write(f.0.path().join("fence/agents/new-agent.toml"),
        "services = ['deepseek']\nwrite = ['${home}/data']\nargs = ['${repo:json}']\nenv = { TMPDIR = '${cwd}' }\n").unwrap();
    fs::write(
        f.0.path().join(".fence.toml"),
        "write = ['${cwd}/../extra']\ndomains = ['example.test']\n",
    )
    .unwrap();
    let out = f
        .command()
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", f.0.path().join("bin").display()),
        )
        .args(["check", "--profile", "dev", "--kind", "new-agent"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], true, "{v}");
    assert_eq!(
        v["settings"]["network"]["allowedDomains"],
        json!([
            "crates.io",
            "index.crates.io",
            "static.crates.io",
            "api.deepseek.com",
            "example.test"
        ])
    );
    assert!(
        v["settings"]["filesystem"]["allowRead"]
            .as_array()
            .unwrap()
            .contains(&json!(f.0.path().join("data")))
    );
    assert_eq!(
        v["args"],
        json!([serde_json::to_string(&f.0.path().canonicalize().unwrap()).unwrap()])
    );
    assert!(
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x.as_str().unwrap().contains("TMPDIR"))
    );
    assert!(
        !f.0.path().join("fence/runs").exists(),
        "check must be read-only"
    );
    fs::write(f.0.path().join(".fence.toml"), "env = { EVIL = 'yes' }").unwrap();
    // No kind means the repository is deliberately not consulted.
    assert_eq!(f.call(&["check", "--profile", "dev"])["ok"], true);
    let out = f
        .command()
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", f.0.path().join("bin").display()),
        )
        .args(["check", "--profile", "dev", "--kind", "new-agent"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["error"], "invalid_data", "{v}");
    assert_eq!(v["problems"][0]["path"], "env");
}

#[test]
fn errors_are_structured_and_bad_overrides_do_not_fall_back() {
    let f = Fixture::new();
    assert_eq!(
        f.call(&["check", "--profile", "absent"])["error"],
        "unknown_profile"
    );
    assert_eq!(
        f.call(&["check", "--profile", "dev", "--kind", "amp"])["error"],
        "unsupported_kind"
    );
    assert_eq!(
        f.call(&["check", "--profile", "dev", "--cwd", "/no-such-fence-cwd"])["error"],
        "bad_cwd"
    );
    assert_eq!(f.call(&["check", "--profile"])["error"], "usage");
    assert_eq!(
        f.call(&["check", "--profile", "dev", "--kind", "codex"])["error"],
        "program_not_found"
    );
    let out = f
        .command()
        .env_remove("FENCE_RUNNER")
        .args(["check", "--profile", "dev"])
        .output()
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["error"],
        "runtime_missing"
    );
    fs::create_dir(f.0.path().join("fence/agents")).unwrap();
    fs::write(
        f.0.path().join("fence/agents/new.toml"),
        "description = 'No service'",
    )
    .unwrap();
    assert_eq!(
        f.call(&["check", "--profile", "dev", "--kind", "new"])["error"],
        "no_service"
    );
    fs::create_dir(f.0.path().join("fence/profiles")).unwrap();
    for text in [
        "write = ['/tmp/*']",
        "write = ['relative']",
        "write = ['${unknown}']",
        "allowAllUnixSockets = true",
        "[linux]\nallow_pty = true",
    ] {
        fs::write(f.0.path().join("fence/profiles/dev.toml"), text).unwrap();
        assert_eq!(
            f.call(&["check", "--profile", "dev"])["error"],
            "invalid_data",
            "{text}"
        );
    }
    let v = f.call(&["profiles"]);
    assert_eq!(v["profiles"][0]["error"]["error"], "invalid_data");
}

fn git(f: &Fixture, cwd: &std::path::Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(cwd)
        .env("HOME", f.0.path())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn git_paths_follow_worktree_branch_detached_and_main_repository() {
    let f = Fixture::new();
    let repo = f.0.path().join("repo");
    fs::create_dir(&repo).unwrap();
    git(&f, &repo, &["init", "-b", "main"]);
    git(
        &f,
        &repo,
        &[
            "-c",
            "user.name=Fence Test",
            "-c",
            "user.email=fence@example.invalid",
            "commit",
            "--allow-empty",
            "-m",
            "initial",
        ],
    );
    let wt = f.0.path().join("worktree");
    git(
        &f,
        &repo,
        &["worktree", "add", "-b", "feature/x", wt.to_str().unwrap()],
    );
    let result = f.call(&["check", "--profile", "dev", "--cwd", wt.to_str().unwrap()]);
    assert_eq!(result["ok"], true, "{result}");
    let repo = repo.canonicalize().unwrap();
    let wt = wt.canonicalize().unwrap();
    let write = result["settings"]["filesystem"]["allowWrite"]
        .as_array()
        .unwrap();
    for path in [
        wt.clone(),
        repo.join(".git/objects"),
        repo.join(".git/worktrees/worktree"),
        repo.join(".git/refs/heads/feature/x.lock"),
    ] {
        assert!(write.contains(&json!(path)), "{result}");
    }
    assert!(!write.contains(&json!(repo)));
    git(&f, &wt, &["checkout", "--detach"]);
    let result = f.call(&["check", "--profile", "dev", "--cwd", wt.to_str().unwrap()]);
    assert!(
        !result["settings"]["filesystem"]["allowWrite"]
            .as_array()
            .unwrap()
            .contains(&json!(repo.join(".git/refs/heads/feature/x.lock")))
    );
    let result = f.call(&["check", "--profile", "dev", "--cwd", repo.to_str().unwrap()]);
    assert!(
        result["settings"]["filesystem"]["allowWrite"]
            .as_array()
            .unwrap()
            .contains(&json!(repo.join(".git")))
    );
}

#[test]
fn four_agents_match_the_development_profile() {
    let f = Fixture::new();
    fs::create_dir(f.0.path().join("bin")).unwrap();
    for kind in ["claude", "codex", "pi", "omp"] {
        std::os::unix::fs::symlink("/bin/sh", f.0.path().join("bin").join(kind)).unwrap();
    }
    let home = f.0.path().canonicalize().unwrap();
    for (kind, domains, agent_write, agent_deny) in [
        (
            "claude",
            vec!["api.anthropic.com", "claude.ai", "platform.claude.com"],
            vec![".claude"],
            vec![".claude/settings.json"],
        ),
        (
            "codex",
            vec![
                "chatgpt.com",
                "ab.chatgpt.com",
                "api.openai.com",
                "auth.openai.com",
            ],
            vec![".codex"],
            vec![".codex/config.toml", ".codex/auth.json"],
        ),
        (
            "pi",
            vec!["api.deepseek.com"],
            vec![
                ".pi/agent/sessions",
                ".pi/agent/auth.json.lock",
                ".pi/agent/settings.json.lock",
            ],
            vec![
                ".pi/agent/settings.json",
                ".pi/agent/auth.json",
                ".pi/agent/models.json",
            ],
        ),
        (
            "omp",
            vec!["api.deepseek.com"],
            vec![".omp/agent", ".omp/natives", ".omp/run"],
            vec![
                ".omp/agent/config.yml",
                ".omp/agent/settings.json",
                ".omp/agent/auth.json",
                ".omp/agent/models.json",
            ],
        ),
    ] {
        let out = f
            .command()
            .env("HOME", &home)
            .env("FENCE_HOME", home.join("fence"))
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", home.join("bin").display()),
            )
            .args(["check", "--profile", "dev", "--kind", kind])
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["ok"], true, "{v}");
        let mut allowed = vec!["crates.io", "index.crates.io", "static.crates.io"];
        allowed.extend(domains);
        let mut write = vec![
            json!(home),
            json!("<运行时才有>"),
            json!(home.join(".cargo/registry")),
            json!(home.join(".cargo/git")),
        ];
        write.extend(agent_write.iter().map(|p| json!(home.join(p))));
        if cfg!(target_os = "linux") && kind == "pi" {
            write.push(json!(home.join(".pi/agent")));
        }
        let mut deny: Vec<_> = agent_deny.iter().map(|p| json!(home.join(p))).collect();
        deny.extend([json!(home.join(".fence.toml")), json!(home.join("fence"))]);
        let mut expected = json!({"network":{"allowedDomains":allowed,"deniedDomains":[]},"filesystem":{"denyRead":[home],"allowRead":write,"allowWrite":write,"denyWrite":deny}});
        if cfg!(target_os = "macos") {
            expected["allowPty"] = json!(true);
            if kind == "codex" {
                expected["enableWeakerNetworkIsolation"] = json!(true);
            }
        }
        assert_eq!(v["settings"], expected, "{kind}");
        assert_eq!(v["env"]["TMPDIR"], "<运行时才有>");
        assert_eq!(
            v["args"],
            if kind == "codex" {
                json!([
                    "-c",
                    format!(
                        "projects={{{}={{trust_level=\"trusted\"}}}}",
                        serde_json::to_string(&home).unwrap()
                    )
                ])
            } else {
                json!([])
            }
        );
    }
}

#[test]
fn run_prepares_private_records_and_denials_is_instance_specific() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let runner = f.0.path().join("runner");
    fs::write(&runner, r#"#!/bin/sh
while [ "$1" != -- ]; do
  case "$1" in --settings) settings=$2;; --denials) denials=$2;; --exit) finished=$2;; esac
  shift 2
done
shift
printf '%s\n' '{"t":1,"type":"network","target":"example.com:443","detail":"denied"}' '{"t":2,"type":"file-write","target":"/blocked","detail":"denied"}' > "$denials"
printf '%s\n' '{"code":7,"signal":null,"t":2}' > "$finished"
printf '%s\n' "$FENCE_RUN" "$TMPDIR" "$@"
exit 7
"#).unwrap();
    fs::set_permissions(&runner, fs::Permissions::from_mode(0o700)).unwrap();
    let out = f
        .command()
        .env("FENCE_RUNNER", &runner)
        .args([
            "run",
            "--profile",
            "dev",
            "--kind",
            "codex",
            "--agent",
            "a/b",
            "--instance",
            "test-1",
            "--",
            "/bin/sh",
            "-c",
            "exit 7",
        ])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(7),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("test-1\n"), "{stdout}");
    assert!(stdout.contains("projects={"), "{stdout}");
    let settings = f.0.path().join("fence/runs/test-1/settings.json");
    assert_eq!(
        fs::metadata(&settings).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let data: Value = serde_json::from_slice(&fs::read(settings).unwrap()).unwrap();
    assert!(
        data["filesystem"]["allowRead"]
            .as_array()
            .unwrap()
            .contains(&json!("/bin/sh"))
    );
    assert!(f.0.path().join(".cache/fence/tmp/test-1").is_dir());
    assert!(!f.0.path().join("fence/runs/test-1/tmp").exists());
    let result = f.call(&[
        "denials",
        "--agent",
        "a/b",
        "--instance",
        "test-1",
        "--limit",
        "1",
    ]);
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["denials"].as_array().unwrap().len(), 1);
    assert_eq!(result["denials"][0]["type"], "file-write");
    assert_eq!(result["running"], false);
    assert_eq!(result["exit"]["code"], 7);
    assert_eq!(f.call(&["denials", "--agent", "a/b"])["instance"], "test-1");
    assert_eq!(
        f.call(&["denials", "--agent", "other", "--instance", "test-1"])["error"],
        "not_found"
    );
    let bad = f
        .command()
        .env("FENCE_RUNNER", &runner)
        .args([
            "run",
            "--profile",
            "dev",
            "--kind",
            "codex",
            "--agent",
            "a/b",
            "--instance",
            "../escape",
            "--",
            "/bin/sh",
        ])
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(125));
    assert!(bad.stdout.is_empty());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("usage"));
}

#[test]
fn expired_runs_remove_the_recorded_cache_directory_but_keep_recent_and_live_runs() {
    let f = Fixture::new();
    let runs = f.0.path().join("fence/runs");
    let old_cache = f.0.path().join("old-cache/fence/tmp");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    for (id, time, pid) in [
        ("old", 1.0, 0),
        ("recent", now, 0),
        ("live", 1.0, std::process::id()),
    ] {
        let dir = runs.join(id);
        let tmp = old_cache.join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("artifact"), "keep until expired").unwrap();
        fs::write(
            dir.join("run.json"),
            json!({"agent":"a","instance":id,"pid":pid,"tmp":tmp,"started":time}).to_string(),
        )
        .unwrap();
        fs::write(
            dir.join("exit.json"),
            json!({"code":0,"t":time}).to_string(),
        )
        .unwrap();
    }
    let new_cache = f.0.path().join("new-cache");
    let out = f
        .command()
        .env("XDG_CACHE_HOME", &new_cache)
        .env("FENCE_RUNNER", "/bin/echo")
        .args([
            "run",
            "--profile",
            "dev",
            "--kind",
            "claude",
            "--agent",
            "new",
            "--instance",
            "new",
            "--",
            "/bin/sh",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(new_cache.join("fence/tmp/new").is_dir());
    assert!(!old_cache.join("old").exists());
    assert!(!runs.join("old").exists());
    for id in ["recent", "live"] {
        assert!(old_cache.join(id).is_dir());
        assert!(runs.join(id).is_dir());
    }
}

#[test]
fn unreadable_override_never_falls_back_to_builtin_rules() {
    let f = Fixture::new();
    let profiles = f.0.path().join("fence/profiles");
    fs::create_dir(&profiles).unwrap();
    std::os::unix::fs::symlink("missing", profiles.join("dev.toml")).unwrap();
    assert_eq!(
        f.call(&["check", "--profile", "dev"])["error"],
        "invalid_data"
    );
    assert_eq!(
        f.call(&["profiles"])["profiles"][0]["error"]["error"],
        "invalid_data"
    );
    fs::remove_file(profiles.join("dev.toml")).unwrap();
    std::os::unix::fs::symlink("missing", f.0.path().join("fence/local.toml")).unwrap();
    assert_eq!(
        f.call(&["check", "--profile", "dev"])["error"],
        "invalid_data"
    );
    fs::remove_file(f.0.path().join("fence/local.toml")).unwrap();
    std::os::unix::fs::symlink("missing", f.0.path().join(".fence.toml")).unwrap();
    assert_eq!(
        f.call(&["check", "--profile", "dev", "--kind", "claude"])["error"],
        "invalid_data"
    );
}
