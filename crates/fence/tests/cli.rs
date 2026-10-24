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
fn kimi_uses_native_research_and_keeps_configuration_read_only() {
    let f = Fixture::new();
    let root = f.0.path().canonicalize().unwrap();
    fs::create_dir(root.join("work")).unwrap();
    fs::create_dir(root.join("bin")).unwrap();
    fs::create_dir_all(root.join(".kimi-code/credentials")).unwrap();
    for path in [
        "config.toml",
        "tui.toml",
        "credentials/kimi-code.json",
        "server.token",
    ] {
        fs::write(root.join(".kimi-code").join(path), "synthetic").unwrap();
    }
    std::os::unix::fs::symlink("/bin/sh", root.join("bin/kimi")).unwrap();
    let out = f
        .command()
        .env("HOME", &root)
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", root.join("bin").display()),
        )
        .current_dir(root.join("work"))
        .args(["check", "--profile", "dev", "--kind", "kimi"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], true, "{v}");
    assert_eq!(v["args"], json!([]));
    for domain in ["api.kimi.com", "auth.kimi.com"] {
        assert!(
            v["settings"]["network"]["allowedDomains"]
                .as_array()
                .unwrap()
                .contains(&json!(domain)),
            "{v}"
        );
    }
    assert!(
        !v["settings"]["network"]["allowedDomains"]
            .as_array()
            .unwrap()
            .contains(&json!("mcp.exa.ai"))
    );
    let files = &v["settings"]["filesystem"];
    let read = files["allowRead"].as_array().unwrap();
    let write = files["allowWrite"].as_array().unwrap();
    let denied = files["denyWrite"].as_array().unwrap();
    for path in [
        "config.toml",
        "tui.toml",
        "credentials",
        "region",
        "server.token",
        "device_id",
        "mcp.json",
        "plugins",
        "AGENTS.md",
        "skills",
        "agents",
        "workspace-trust",
        "bin",
    ] {
        assert!(
            denied.contains(&json!(root.join(".kimi-code").join(path))),
            "{v}"
        );
    }
    assert!(write.contains(&json!(root.join(".kimi-code"))));
    assert!(read.contains(&json!(root.join(".kimi-code"))));
    assert_eq!(v["env"]["KIMI_CODE_CACHE_DIR"], "<运行时才有>/kimi-cache");
    assert_eq!(v["env"]["KIMI_CODE_WATCH"], "0");
}

#[test]
fn kind_without_mcp_template_skips_servers_but_still_protects_keys() {
    let f = Fixture::new();
    let root = f.0.path();
    fs::create_dir(root.join("fence/agents")).unwrap();
    fs::write(
        root.join("fence/agents/native.toml"),
        "services = ['deepseek']\n",
    )
    .unwrap();
    fs::create_dir(root.join("bin")).unwrap();
    std::os::unix::fs::symlink("/bin/sh", root.join("bin/native")).unwrap();
    fs::create_dir(root.join("fence/keys")).unwrap();
    fs::write(root.join("private-key"), "synthetic-key").unwrap();
    std::os::unix::fs::symlink(root.join("private-key"), root.join("fence/keys/exa")).unwrap();
    let out = f
        .command()
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", root.join("bin").display()),
        )
        .args(["check", "--profile", "dev", "--kind", "native"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], true, "{v}");
    assert!(
        !v["settings"]["network"]["allowedDomains"]
            .as_array()
            .unwrap()
            .contains(&json!("mcp.exa.ai")),
        "{v}"
    );
    assert_eq!(v["args"], json!([]));
    assert!(
        !v["layers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["source"] == "mcp")
    );
    assert!(
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| { w.as_str().unwrap().contains("mcp_skipped: native") })
    );
    let denied = v["settings"]["filesystem"]["denyRead"].as_array().unwrap();
    assert!(denied.contains(&json!(root.join("fence/keys/exa"))));
    assert!(denied.contains(&json!(root.join("private-key").canonicalize().unwrap())));
    // Without a kind, check still validates and previews the profile's MCP selection.
    let profile = f.call(&["check", "--profile", "dev"]);
    assert!(
        profile["layers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["source"] == "mcp")
    );
}

#[test]
fn dev_gives_every_kind_its_own_srt_temporary_directory() {
    let f = Fixture::new();
    let bin = f.0.path().join("bin");
    fs::create_dir(&bin).unwrap();
    for kind in ["claude", "codex", "pi", "omp", "kimi"] {
        std::os::unix::fs::symlink("/bin/sh", bin.join(kind)).unwrap();
        let out = f
            .command()
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .args(["check", "--profile", "dev", "--kind", kind])
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["ok"], true, "{kind}: {v}");
        assert_eq!(v["env"]["TMPDIR"], "<运行时才有>");
        assert_eq!(v["env"]["CLAUDE_CODE_TMPDIR"], "<运行时才有>", "{kind}");
    }
}

#[test]
fn dev_only_allows_unix_sockets_in_its_temporary_directory() {
    let f = Fixture::new();
    let v = f.call(&["check", "--profile", "dev"]);
    assert_eq!(v["ok"], true, "{v}");
    if cfg!(target_os = "macos") {
        assert_eq!(
            v["settings"]["network"]["allowUnixSockets"],
            json!(["<运行时才有>"])
        );
    } else {
        assert!(v["settings"]["network"].get("allowUnixSockets").is_none());
        assert!(v["warnings"].as_array().unwrap().iter().any(|w| {
            w.as_str()
                .unwrap()
                .contains("Linux 不能按路径放行 unix socket，已忽略")
        }));
    }
    assert!(
        v["settings"]["network"]
            .get("allowAllUnixSockets")
            .is_none()
    );
}

#[test]
fn local_unix_sockets_resolve_links_and_missing_suffixes_without_creating_paths() {
    let f = Fixture::new();
    let root = f.0.path();
    fs::create_dir(root.join("actual")).unwrap();
    let actual = root.join("actual").canonicalize().unwrap();
    std::os::unix::fs::symlink("actual", root.join("hop")).unwrap();
    std::os::unix::fs::symlink("hop", root.join("alias")).unwrap();
    let tmp = tempfile::tempdir_in("/tmp").unwrap();
    let canonical_tmp = tmp.path().canonicalize().unwrap();
    fs::write(
        root.join("fence/local.toml"),
        format!(
            "unix_sockets = ['${{home}}/alias/missing/sockets', '{}/new', '{}/new']\n",
            tmp.path().display(),
            canonical_tmp.display()
        ),
    )
    .unwrap();
    let v = f.call(&["check", "--profile", "dev"]);
    assert_eq!(v["ok"], true, "{v}");
    if cfg!(target_os = "macos") {
        assert_eq!(
            v["settings"]["network"]["allowUnixSockets"],
            json!([
                "<运行时才有>",
                actual.join("missing/sockets"),
                canonical_tmp.join("new")
            ])
        );
    } else {
        assert!(v["settings"]["network"].get("allowUnixSockets").is_none());
    }
    assert!(!actual.join("missing").exists());
    assert!(!canonical_tmp.join("new").exists());

    for target in ["missing-target", "alias"] {
        fs::remove_file(root.join("hop")).unwrap();
        std::os::unix::fs::symlink(target, root.join("hop")).unwrap();
        let v = f.call(&["check", "--profile", "dev"]);
        assert_eq!(v["error"], "invalid_data", "{v}");
        assert_eq!(v["problems"][0]["path"], "unix_sockets");
    }
}

#[test]
fn unix_sockets_reject_wildcards_relative_paths_and_repo_permissions() {
    let f = Fixture::new();
    for path in ["/tmp/*", "/tmp/?", "/tmp/[ab]", "relative/socket"] {
        fs::write(
            f.0.path().join("fence/local.toml"),
            format!("unix_sockets = [{path:?}]\n"),
        )
        .unwrap();
        let v = f.call(&["check", "--profile", "dev"]);
        assert_eq!(v["error"], "invalid_data", "{path}: {v}");
        assert_eq!(v["problems"][0]["path"], "unix_sockets");
    }
    fs::remove_file(f.0.path().join("fence/local.toml")).unwrap();
    fs::write(
        f.0.path().join(".fence.toml"),
        "unix_sockets = ['${tmp}']\n",
    )
    .unwrap();
    let v = f.call(&["check", "--profile", "dev", "--kind", "pi"]);
    assert_eq!(v["error"], "invalid_data", "{v}");
    assert_eq!(v["problems"][0]["path"], "unix_sockets");
    assert!(
        v["problems"][0]["source"]
            .as_str()
            .unwrap()
            .ends_with("/.fence.toml")
    );
}

#[test]
fn runtime_failures_keep_messages_but_agent_exit_125_does_not() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    fs::create_dir(f.0.path().join("fence/agents")).unwrap();
    // This fixture launches /bin/sh, not Claude's CLI.
    fs::write(
        f.0.path().join("fence/agents/claude.toml"),
        "services = ['anthropic']\n",
    )
    .unwrap();
    let root = f.0.path();
    let module = root.join("lib/srt/node_modules/@anthropic-ai/sandbox-runtime");
    fs::create_dir_all(module.join("dist")).unwrap();
    fs::create_dir_all(root.join("share/fence")).unwrap();
    fs::write(module.join("package.json"), r#"{"type":"module"}"#).unwrap();
    fs::write(
        module.join("dist/index.js"),
        r#"
export const SandboxRuntimeConfigSchema = { parse: value => value };
export const SandboxManager = {
  getSandboxViolationStore: () => ({ subscribe: () => {} }),
  initialize: async () => {
    if (process.env.FAIL_SRT) throw new Error('srt initialization failed');
  },
  wrapWithSandbox: async command => "/bin/sh -c '" + command.replaceAll("'", "'\\''") + "'",
  reset: async () => {}
};
"#,
    )
    .unwrap();
    let runner = root.join("share/fence/runner.mjs");
    fs::copy(
        concat!(env!("CARGO_MANIFEST_DIR"), "/resources/runner.mjs"),
        &runner,
    )
    .unwrap();
    let node = Command::new("node")
        .args(["-p", "process.execPath"])
        .output()
        .unwrap();
    assert!(node.status.success());
    let wrapper = root.join("runner");
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nexec '{}' '{}' \"$@\"\n",
            String::from_utf8(node.stdout).unwrap().trim(),
            runner.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
    let broken = root.join("broken-runner");
    fs::write(&broken, "#!/nonexistent/fence-interpreter\n").unwrap();
    fs::set_permissions(&broken, fs::Permissions::from_mode(0o700)).unwrap();
    for (id, executable, fail_srt, expected) in [
        (
            "srt-error",
            &wrapper,
            true,
            Some("srt initialization failed"),
        ),
        (
            "exec-error",
            &broken,
            false,
            Some("No such file or directory"),
        ),
        ("agent-125", &wrapper, false, None),
    ] {
        let mut command = f.command();
        command.env("FENCE_RUNNER", executable);
        if fail_srt {
            command.env("FAIL_SRT", "1");
        }
        let out = command
            .args([
                "run",
                "--profile",
                "dev",
                "--kind",
                "claude",
                "--agent",
                "test/errors",
                "--instance",
                id,
                "--",
                "/bin/sh",
                "-c",
                "exit 125",
                "private-first-prompt",
            ])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(125), "{out:?}");
        let result = f.call(&["denials", "--agent", "test/errors", "--instance", id]);
        assert_eq!(result["exit"]["code"], 125, "{result}");
        assert!(result["exit"]["t"].is_number());
        assert_eq!(result["exit"]["signal"], Value::Null);
        match expected {
            Some(message) => assert!(
                result["exit"]["message"]
                    .as_str()
                    .is_some_and(|m| m.contains(message)),
                "{result}"
            ),
            None => assert!(result["exit"].get("message").is_none(), "{result}"),
        }
        assert!(!result.to_string().contains("private-first-prompt"));
        assert!(!result.to_string().contains("exit 125"));
    }
}

#[test]
fn denial_counts_cover_all_recorded_types_independently_of_limit() {
    let f = Fixture::new();
    let dir = f.0.path().join("fence/runs/counts");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("run.json"),
        r#"{"agent":"test/counts","instance":"counts","started":1,"pid":0}"#,
    )
    .unwrap();
    fs::write(
        dir.join("monitor.json"),
        r#"{"monitors":{"network":true,"files":true},"truncated":true}"#,
    )
    .unwrap();
    fs::write(dir.join("denials.jsonl"), "").unwrap();
    let empty = f.call(&["denials", "--agent", "test/counts"]);
    assert_eq!(
        empty["counts"],
        json!({"network":0,"file-read":0,"file-write":0,"other":0})
    );
    let entries = ["network", "network", "file-write", "future-type", "other"];
    fs::write(
        dir.join("denials.jsonl"),
        entries
            .map(|kind| format!("{}\n", json!({"type":kind})))
            .concat(),
    )
    .unwrap();
    for limit in ["0", "1", "200"] {
        let result = f.call(&[
            "denials",
            "--agent",
            "test/counts",
            "--instance",
            "counts",
            "--limit",
            limit,
        ]);
        assert_eq!(
            result["counts"],
            json!({"network":2,"file-read":0,"file-write":1,"other":1,"future-type":1})
        );
        assert_eq!(result["truncated"], true);
        let expected = match limit {
            "0" => json!([]),
            "1" => json!([{"type":"other"}]),
            _ => json!(entries.map(|kind| json!({"type":kind}))),
        };
        assert_eq!(result["denials"], expected);
    }
}

#[test]
fn deny_write_resolves_linked_parents_and_rejects_unresolvable_targets() {
    let f = Fixture::new();
    let root = f.0.path();
    fs::create_dir(root.join("actual")).unwrap();
    let actual = root.join("actual").canonicalize().unwrap();
    std::os::unix::fs::symlink(root.join("hop"), root.join("alias")).unwrap();
    std::os::unix::fs::symlink("actual", root.join("hop")).unwrap();
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
    for node in ["alias", "hop"] {
        assert!(denies.contains(&json!(root.canonicalize().unwrap().join(node))));
    }
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
    for node in ["alias", "hop"] {
        assert!(
            v["settings"]["filesystem"]["denyWrite"]
                .as_array()
                .unwrap()
                .contains(&json!(root.canonicalize().unwrap().join(node)))
        );
    }
    std::os::unix::fs::symlink("alias", root.join("cycle-hop")).unwrap();
    for target in ["absent", "alias", "cycle-hop"] {
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
    assert_eq!(result["kinds"].as_array().unwrap().len(), 5);
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
            "github.com",
            "api.github.com",
            "raw.githubusercontent.com",
            "codeload.github.com",
            "objects.githubusercontent.com",
            "docs.rs",
            "doc.rust-lang.org",
            "static.rust-lang.org",
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
            vec!["api.deepseek.com", "opencode.ai"],
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
            vec!["api.deepseek.com", "opencode.ai"],
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
        let mut allowed = vec![
            "crates.io",
            "index.crates.io",
            "static.crates.io",
            "github.com",
            "api.github.com",
            "raw.githubusercontent.com",
            "codeload.github.com",
            "objects.githubusercontent.com",
            "docs.rs",
            "doc.rust-lang.org",
            "static.rust-lang.org",
        ];
        allowed.extend(domains);
        allowed.push("mcp.exa.ai");
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
        let mut expected = json!({"network":{"allowedDomains":allowed,"deniedDomains":[]},"filesystem":{"denyRead":[home,home.join("fence/keys"),home.join("fence/keys/exa")],"allowRead":write,"allowWrite":write,"denyWrite":deny}});
        if cfg!(target_os = "macos") {
            expected["network"]["allowUnixSockets"] = json!(["<运行时才有>"]);
            expected["allowPty"] = json!(true);
            if kind == "codex" {
                expected["enableWeakerNetworkIsolation"] = json!(true);
            }
        }
        assert_eq!(v["settings"], expected, "{kind}");
        assert_eq!(v["env"]["TMPDIR"], "<运行时才有>");
        assert_eq!(
            v["args"],
            match kind {
                "codex" => json!([
                    "-c",
                    format!(
                        "projects={{{}={{trust_level=\"trusted\"}}}}",
                        serde_json::to_string(&home).unwrap()
                    ),
                    "-c",
                    "mcp_servers.fence_exa_<运行时才有>={url=\"https://mcp.exa.ai/mcp?tools=web_search_exa,web_fetch_exa\",env_http_headers={}}"
                ]),
                "claude" => json!(["--mcp-config", "<运行时才有>/mcp/exa/mcp.json"]),
                "pi" => json!(["--extension", "<运行时才有>/mcp/exa/pi.ts"]),
                "omp" => json!(["--plugin-dir", "<运行时才有>/mcp/exa"]),
                _ => unreachable!(),
            }
        );
    }
}

#[test]
fn run_passes_research_arguments_and_writes_session_resources() {
    research_configuration("default");
}

#[test]
fn local_mcp_selection_switches_all_four_clients_without_replacing_agent_data() {
    research_configuration("replacement");
}

#[test]
fn mcp_key_is_scoped_to_its_server_for_all_four_clients() {
    research_configuration("key");
}

fn research_configuration(scenario: &str) {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let root = f.0.path().canonicalize().unwrap();
    let keyed = scenario == "key";
    let servers: &[(&str, &str)] = match scenario {
        "default" => &[(
            "exa",
            "https://mcp.exa.ai/mcp?tools=web_search_exa,web_fetch_exa",
        )],
        "replacement" => &[("research", "https://research.example.invalid:8443/mcp")],
        "key" => &[
            ("research", "https://research.example.invalid:8443/mcp"),
            (
                "exa",
                "https://mcp.exa.ai/mcp?tools=web_search_exa,web_fetch_exa",
            ),
        ],
        _ => unreachable!(),
    };
    let key = "fake-only-\"quoted\"-\\value";
    if scenario != "default" {
        fs::create_dir(root.join("fence/mcp")).unwrap();
        fs::write(
            root.join("fence/mcp/research.toml"),
            "url = 'https://research.example.invalid:8443/mcp'\n[auth]\nheader = 'X-Research-Key'\nprefix = 'Token '\n",
        ).unwrap();
        fs::write(
            root.join("fence/local.toml"),
            if keyed {
                "mcp = ['research', 'exa']\n"
            } else {
                "mcp = ['research']\n"
            },
        )
        .unwrap();
    }
    if keyed {
        fs::create_dir(root.join("fence/keys")).unwrap();
        fs::write(root.join("fence/keys/research"), format!("{key}\n")).unwrap();
        fs::set_permissions(
            root.join("fence/keys/research"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
    }
    let runner = root.join("record-argv");
    fs::create_dir(root.join("bin")).unwrap();
    for kind in ["claude", "codex", "pi", "omp"] {
        std::os::unix::fs::symlink("/bin/sh", root.join("bin").join(kind)).unwrap();
    }
    fs::write(
        &runner,
        "#!/bin/sh\n/usr/bin/env > \"$TMPDIR/runner-env\"\nwhile [ \"$1\" != -- ]; do shift 2; done\nshift\nprintf '%s\\000' \"$@\"\n",
    )
    .unwrap();
    fs::set_permissions(&runner, fs::Permissions::from_mode(0o700)).unwrap();
    let mut codex_names = Vec::new();
    for kind in ["claude", "codex", "pi", "omp", "codex"] {
        let instance = format!("research-{kind}-{}", codex_names.len());
        let out = f
            .command()
            .env("HOME", &root)
            .env("FENCE_HOME", root.join("fence"))
            .env("FENCE_RUNNER", &runner)
            .args([
                "run",
                "--profile",
                "dev",
                "--kind",
                kind,
                "--agent",
                "test/research",
                "--instance",
                &instance,
                "--",
                "/bin/sh",
                "user argument",
            ])
            .output()
            .unwrap();
        assert!(out.status.success(), "{kind}: {out:?}");
        let text = String::from_utf8(out.stdout).unwrap();
        let argv: Vec<_> = text.split_terminator('\0').collect();
        let tmp = root.join(".cache/fence/tmp").join(&instance);
        let environment = fs::read_to_string(tmp.join("runner-env")).unwrap();
        assert!(
            environment
                .lines()
                .any(|s| s == format!("TMPDIR={}", tmp.display()))
        );
        assert!(
            environment
                .lines()
                .any(|s| s == format!("FENCE_RUN={instance}"))
        );
        assert!(
            !text.contains("fake-only"),
            "credential must never enter argv"
        );
        assert!(!String::from_utf8_lossy(&out.stderr).contains("fake-only"));
        let secret_env: Vec<_> = environment
            .lines()
            .filter(|line| line.starts_with("FENCE_MCP_"))
            .collect();
        assert_eq!(secret_env.len(), usize::from(keyed && kind == "codex"));
        if kind != "codex" {
            assert!(!environment.contains(key));
        }
        assert_eq!(argv[0], "/bin/sh");
        assert_eq!(argv.last(), Some(&"user argument"));
        let start = if kind == "codex" {
            assert_eq!(argv[1], "-c");
            let project: toml::Table = toml::from_str(argv[2]).unwrap();
            assert_eq!(
                project["projects"][root.to_str().unwrap()]["trust_level"].as_str(),
                Some("trusted")
            );
            3
        } else {
            1
        };
        assert_eq!(argv.len(), start + servers.len() * 2 + 1);
        for (index, (name, url)) in servers.iter().enumerate() {
            let headers = if keyed && *name == "research" {
                json!({"X-Research-Key": format!("Token {key}")})
            } else {
                json!({})
            };
            let pair = &argv[start + index * 2..start + index * 2 + 2];
            let dir = tmp.join("mcp").join(name);
            let server_name = format!("fence_{name}");
            match kind {
                "claude" => {
                    assert_eq!(pair[0], "--mcp-config");
                    let path = dir.join("mcp.json");
                    assert_eq!(pair[1], path.to_str().unwrap());
                    let config: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                    assert_eq!(
                        fs::metadata(path).unwrap().permissions().mode() & 0o777,
                        0o600
                    );
                    assert_eq!(
                        config,
                        json!({"mcpServers":{server_name:{"type":"http","url":url,"headers":headers}}})
                    );
                }
                "codex" => {
                    assert_eq!(pair[0], "-c");
                    let config: toml::Table = toml::from_str(pair[1]).unwrap();
                    let table = config["mcp_servers"].as_table().unwrap();
                    assert_eq!(table.len(), 1);
                    let (actual_name, server) = table.iter().next().unwrap();
                    let nonce = actual_name
                        .strip_prefix(&format!("{server_name}_"))
                        .expect("per-run MCP name");
                    assert_eq!(nonce.len(), 32);
                    assert!(nonce.bytes().all(|b| b.is_ascii_hexdigit()));
                    assert!(
                        !codex_names.contains(actual_name),
                        "each run needs a fresh name"
                    );
                    codex_names.push(actual_name.clone());
                    let refs = server["env_http_headers"].as_table().unwrap();
                    let expected_refs = if keyed && *name == "research" {
                        let variable = refs["X-Research-Key"].as_str().unwrap();
                        assert!(variable.starts_with(&format!("FENCE_MCP_{nonce}_")));
                        assert_eq!(secret_env, [format!("{variable}=Token {key}")]);
                        json!({"X-Research-Key":variable})
                    } else {
                        json!({})
                    };
                    assert_eq!(
                        serde_json::to_value(server).unwrap(),
                        json!({"url":url,"env_http_headers":expected_refs})
                    );
                }
                "pi" => {
                    let path = dir.join("pi.ts");
                    assert_eq!(pair, ["--extension", path.to_str().unwrap()]);
                    let script = fs::read_to_string(&path).unwrap();
                    assert_eq!(
                        script,
                        format!(
                            "export default function (pi) {{\n  pi.registerMcpServer({}, {{\n    url: {},\n    headers: {{ Authorization: \"\", ...{} }},\n    exposure: \"direct\",\n  }});\n}}\n",
                            json!(server_name),
                            json!(url),
                            headers,
                        )
                    );
                    assert_eq!(
                        fs::metadata(path).unwrap().permissions().mode() & 0o777,
                        0o600
                    );
                }
                "omp" => {
                    assert_eq!(pair, ["--plugin-dir", dir.to_str().unwrap()]);
                    let manifest: Value = serde_json::from_slice(
                        &fs::read(dir.join(".claude-plugin/plugin.json")).unwrap(),
                    )
                    .unwrap();
                    assert_eq!(manifest, json!({"name":format!("fence-{name}")}));
                    let path = dir.join(".mcp.json");
                    let config: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                    assert_eq!(
                        config,
                        json!({"mcpServers":{server_name:{"type":"http","url":url,"headers":headers}}})
                    );
                    assert_eq!(
                        fs::metadata(path).unwrap().permissions().mode() & 0o777,
                        0o600
                    );
                }
                _ => unreachable!(),
            }
        }
        let record = root.join("fence/runs").join(&instance);
        let settings: Value =
            serde_json::from_slice(&fs::read(record.join("settings.json")).unwrap()).unwrap();
        let domains = settings["network"]["allowedDomains"].as_array().unwrap();
        assert_eq!(
            domains.contains(&json!("mcp.exa.ai")),
            scenario != "replacement"
        );
        assert_eq!(
            domains.contains(&json!("research.example.invalid:8443")),
            scenario != "default"
        );
        assert!(
            settings["filesystem"]["denyRead"]
                .as_array()
                .unwrap()
                .contains(&json!(root.join("fence/keys")))
        );
        for file in ["settings.json", "run.json"] {
            assert!(
                !fs::read_to_string(record.join(file))
                    .unwrap()
                    .contains("fake-only")
            );
        }
        let check = f
            .command()
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", root.join("bin").display()),
            )
            .args(["check", "--profile", "dev", "--kind", kind])
            .output()
            .unwrap();
        assert!(check.status.success(), "{check:?}");
        let preview: Value = serde_json::from_slice(&check.stdout).unwrap();
        assert!(!preview.to_string().contains("fake-only"));
        assert!(
            !preview["env"]
                .as_object()
                .unwrap()
                .keys()
                .any(|k| k.starts_with("FENCE_MCP_"))
        );
        let denials = f.call(&[
            "denials",
            "--agent",
            "test/research",
            "--instance",
            &instance,
        ]);
        assert!(!denials.to_string().contains("fake-only"));
    }
    let checked = f.call(&["check", "--profile", "dev"]);
    assert_eq!(checked["ok"], true, "{checked}");
    assert!(!checked.to_string().contains(key));
}

fn mcp_review_fixture() -> Fixture {
    let f = Fixture::new();
    let root = f.0.path();
    fs::create_dir(root.join("fence/mcp")).unwrap();
    fs::create_dir(root.join("fence/keys")).unwrap();
    fs::write(root.join("fence/mcp/research.toml"),
        "url = 'https://research.example.invalid/mcp'\n[auth]\nheader = 'Authorization'\nprefix = 'Token '\n").unwrap();
    fs::write(root.join("fence/local.toml"), "mcp = ['research']\n").unwrap();
    f
}

#[test]
fn mcp_rejects_dollar_interpolation_before_starting_a_runner() {
    let f = mcp_review_fixture();
    for key in ["$REVIEW_OTHER_TOKEN", "${REVIEW_OTHER_TOKEN}", "abc$$def"] {
        fs::write(f.0.path().join("fence/keys/research"), key).unwrap();
        let out = f
            .command()
            .env("REVIEW_OTHER_TOKEN", "fake-other-provider-secret")
            .env("FENCE_RUNNER", "/bin/echo")
            .args([
                "run",
                "--profile",
                "dev",
                "--kind",
                "pi",
                "--agent",
                "test/review",
                "--instance",
                "dollar",
                "--",
                "/bin/sh",
            ])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(125), "{key}: {out:?}");
        assert!(out.stdout.is_empty());
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(error.contains("literal HTTP header"), "{error}");
        assert!(!error.contains(key));
        assert!(!error.contains("fake-other-provider-secret"));
        assert!(!f.0.path().join("fence/runs/dollar").exists());
    }
    // The prefix participates in the same client interpolation as the key.
    fs::write(f.0.path().join("fence/keys/research"), "fake-literal").unwrap();
    fs::write(f.0.path().join("fence/mcp/research.toml"),
        "url = 'https://research.example.invalid/mcp'\n[auth]\nheader = 'Authorization'\nprefix = '$REVIEW_OTHER_TOKEN '\n").unwrap();
    let out = f
        .command()
        .env("FENCE_RUNNER", "/bin/echo")
        .args([
            "run",
            "--profile",
            "dev",
            "--kind",
            "pi",
            "--agent",
            "test/review",
            "--instance",
            "prefix",
            "--",
            "/bin/sh",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(125), "{out:?}");
    assert!(out.stdout.is_empty());
}

#[test]
fn mcp_protects_all_key_link_targets_even_after_switching_or_disabling_servers() {
    use std::os::unix::fs::symlink;
    let f = mcp_review_fixture();
    let root = f.0.path();
    fs::create_dir(root.join("bin")).unwrap();
    symlink("/bin/sh", root.join("bin/pi")).unwrap();
    let work = tempfile::tempdir().unwrap();
    let target = work.path().join("provider-token.txt");
    // Invalid UTF-8 proves check does not try to read key content.
    fs::write(&target, [0xff]).unwrap();
    symlink(&target, root.join("fence/keys/research")).unwrap();
    for directory_link in [false, true] {
        if directory_link {
            fs::rename(root.join("fence/keys"), root.join("key-storage")).unwrap();
            symlink(root.join("key-storage"), root.join("fence/keys")).unwrap();
        }
        for selection in ["['research']", "['exa']", "[]"] {
            fs::write(
                root.join("fence/local.toml"),
                format!("mcp = {selection}\n"),
            )
            .unwrap();
            let out = f
                .command()
                .env(
                    "PATH",
                    format!("{}:/usr/bin:/bin", root.join("bin").display()),
                )
                .args([
                    "check",
                    "--profile",
                    "dev",
                    "--kind",
                    "pi",
                    "--cwd",
                    work.path().to_str().unwrap(),
                ])
                .output()
                .unwrap();
            assert!(out.status.success(), "{out:?}");
            let checked: Value = serde_json::from_slice(&out.stdout).unwrap();
            assert_eq!(checked["ok"], true, "{checked}");
            let denies = checked["settings"]["filesystem"]["denyRead"]
                .as_array()
                .unwrap();
            assert!(
                denies.contains(&json!(target.canonicalize().unwrap())),
                "{selection}: {checked}"
            );
            if directory_link {
                assert!(denies.contains(&json!(root.join("key-storage").canonicalize().unwrap())));
            }
        }
    }
    // An unselected invalid key must stay protected without being read by run, too.
    let out = f
        .command()
        .env("FENCE_RUNNER", "/bin/echo")
        .current_dir(work.path())
        .args([
            "run",
            "--profile",
            "dev",
            "--kind",
            "pi",
            "--agent",
            "test/review",
            "--instance",
            "unselected",
            "--",
            "/bin/sh",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
}

#[test]
fn mcp_preparation_failure_cleans_only_directories_created_by_this_run() {
    let f = mcp_review_fixture();
    let root = f.0.path();
    fs::write(root.join("fence/keys/research"), "fake-preparation-key").unwrap();
    fs::write(root.join("not-a-directory"), "keep").unwrap();
    fs::write(
        root.join("fence/local.toml"),
        "mcp = ['research']\nmkdir = ['${home}/not-a-directory/child']\n",
    )
    .unwrap();
    for kind in ["claude", "pi", "omp"] {
        let out = f
            .command()
            .env("FENCE_RUNNER", "/bin/echo")
            .args([
                "run",
                "--profile",
                "dev",
                "--kind",
                kind,
                "--agent",
                "test/review",
                "--instance",
                kind,
                "--",
                "/bin/sh",
            ])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(125), "{out:?}");
        assert!(out.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&out.stderr).contains("fake-preparation-key"));
        assert!(
            !root.join(".cache/fence/tmp").join(kind).exists(),
            "{kind}: secret resources remain"
        );
        assert!(!root.join("fence/runs").join(kind).exists());
    }
    for existing in ["record", "tmp-dir", "tmp-file", "tmp-link"] {
        let dir = root.join("fence/runs").join(existing);
        let tmp = root.join(".cache/fence/tmp").join(existing);
        let original = if existing == "record" { &dir } else { &tmp };
        fs::create_dir_all(original.parent().unwrap()).unwrap();
        if existing == "tmp-link" {
            std::os::unix::fs::symlink(root.join("not-a-directory"), original).unwrap();
        } else if existing == "tmp-file" {
            fs::write(original, "original").unwrap();
        } else {
            fs::create_dir(original).unwrap();
            fs::write(original.join("keep"), "original").unwrap();
        }
        let out = f
            .command()
            .env("FENCE_RUNNER", "/bin/echo")
            .args([
                "run",
                "--profile",
                "dev",
                "--kind",
                "pi",
                "--agent",
                "test/review",
                "--instance",
                existing,
                "--",
                "/bin/sh",
            ])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(125), "{out:?}");
        if existing == "tmp-link" {
            assert!(fs::symlink_metadata(original).unwrap().is_symlink());
        } else if existing == "tmp-file" {
            assert_eq!(fs::read_to_string(original).unwrap(), "original");
        } else {
            assert_eq!(
                fs::read_to_string(original.join("keep")).unwrap(),
                "original"
            );
        }
        if existing != "record" {
            assert!(!dir.exists());
        }
    }
    assert_eq!(
        fs::read_to_string(root.join("not-a-directory")).unwrap(),
        "keep"
    );
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
