//! `corral integration status`, with an isolated HOME and synthetic agent programs on PATH.
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{Duration, Instant},
};

struct Lab {
    root: tempfile::TempDir,
}
impl Lab {
    fn new() -> Self {
        let root = tempfile::tempdir_in("/tmp").unwrap();
        fs::create_dir(root.path().join("bin")).unwrap();
        Self { root }
    }
    fn path(&self, rel: &str) -> PathBuf {
        self.root.path().join(rel)
    }
    fn cmd(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_corral"));
        c.args(args)
            .env("HOME", self.root.path())
            .env("CORRAL_HOME", self.path("pens"))
            .env(
                "PATH",
                format!("relative:{}:/usr/bin:/bin", self.path("bin").display()),
            )
            .env_remove("CLAUDE_CONFIG_DIR")
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("CODEX_SANDBOX");
        c
    }
    fn out(&self, args: &[&str]) -> Output {
        self.cmd(args).output().unwrap()
    }
    fn status(&self) -> Value {
        let o = self.out(&["integration", "status"]);
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
        serde_json::from_slice(&o.stdout).unwrap()
    }
    fn program(&self, name: &str, script: &str) {
        let path = self.path("bin").join(name);
        fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fn write(&self, rel: &str, body: &str) {
        let path = self.path(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }
    /// Every file and symlink under HOME except corral's own namespace, with its content.
    fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fn walk(dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                let meta = fs::symlink_metadata(&path).unwrap();
                if meta.file_type().is_symlink() {
                    out.insert(
                        path.clone(),
                        fs::read_link(&path)
                            .unwrap()
                            .into_os_string()
                            .into_encoded_bytes(),
                    );
                } else if meta.is_dir() {
                    out.insert(path.clone(), vec![]);
                    walk(&path, out);
                } else {
                    out.insert(path.clone(), fs::read(&path).unwrap());
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(self.root.path(), &mut out);
        out
    }
}
fn alive(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}

#[test]
fn lists_every_registered_agent_in_order_even_when_none_is_installed() {
    let lab = Lab::new();
    let s = lab.status();
    let home = lab.root.path().display().to_string();
    assert_eq!(
        s,
        json!({"ok":true,"agents":[
            {"kind":"claude","path":null,"version":null,"hooks":"launch","skills":[
                {"target":"claude","path":format!("{home}/.claude/skills/corral/SKILL.md"),"status":"create"}]},
            {"kind":"codex","path":null,"version":null,"hooks":"launch","skills":[
                {"target":"codex","path":format!("{home}/.agents/skills/corral/SKILL.md"),"status":"create"}]},
            {"kind":"pi","path":null,"version":null,"hooks":"launch","skills":[
                {"target":"codex","path":format!("{home}/.agents/skills/corral/SKILL.md"),"status":"create"}]},
            {"kind":"omp","path":null,"version":null,"hooks":"launch","skills":[
                {"target":"claude","path":format!("{home}/.claude/skills/corral/SKILL.md"),"status":"create"},
                {"target":"codex","path":format!("{home}/.agents/skills/corral/SKILL.md"),"status":"create"}]},
            {"kind":"amp","path":null,"version":null,"hooks":"global",
             "install":{"path":format!("{home}/.config/amp/plugins/corral.ts"),"status":"create","version":null},
             "skills":[
                {"target":"claude","path":format!("{home}/.claude/skills/corral/SKILL.md"),"status":"create"},
                {"target":"codex","path":format!("{home}/.agents/skills/corral/SKILL.md"),"status":"create"}]},
            {"kind":"agy","path":null,"version":null,"hooks":"global",
             "install":{"path":format!("{home}/.gemini/config/plugins/corral"),"status":"create","version":null,"enabled":true},
             "skills":[
                {"target":"gemini","path":format!("{home}/.gemini/config/skills/corral/SKILL.md"),"status":"create"}]},
        ]})
    );
}

#[test]
fn finds_programs_and_reads_versions_without_hanging() {
    let lab = Lab::new();
    // Not executable, and in a relative PATH entry: neither counts.
    lab.write("relative/claude", "#!/bin/sh\necho wrong\n");
    lab.program("claude", "printf '\\n  1.0.0 (Claude Code)  \\nmore\\n'");
    lab.program("codex", "echo codex-cli 9.9; exit 1");
    lab.program("pi", "exit 0");
    let pids = lab.path("omp.pids");
    lab.program(
        "omp",
        &format!("sleep 60 &\necho \"$$ $!\" > {}\nsleep 60", pids.display()),
    );
    let started = Instant::now();
    let s = lab.status();
    let took = started.elapsed();
    assert!(took < Duration::from_secs(8), "took {took:?}");
    assert!(took >= Duration::from_secs(2), "took {took:?}");
    let bin = lab.path("bin").display().to_string();
    let found: Vec<_> = s["agents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| (a["kind"].clone(), a["path"].clone(), a["version"].clone()))
        .collect();
    assert_eq!(
        found,
        vec![
            (
                json!("claude"),
                json!(format!("{bin}/claude")),
                json!("1.0.0 (Claude Code)")
            ),
            (json!("codex"), json!(format!("{bin}/codex")), Value::Null),
            (json!("pi"), json!(format!("{bin}/pi")), Value::Null),
            (json!("omp"), json!(format!("{bin}/omp")), Value::Null),
            (json!("amp"), Value::Null, Value::Null),
            (json!("agy"), Value::Null, Value::Null),
        ]
    );
    // The hanging program and the child it left behind were both killed.
    let text = fs::read_to_string(&pids).unwrap();
    let ids: Vec<i32> = text
        .split_whitespace()
        .map(|p| p.parse().unwrap())
        .collect();
    assert_eq!(ids.len(), 2);
    let end = Instant::now() + Duration::from_secs(2);
    while ids.iter().any(|p| alive(*p)) {
        assert!(Instant::now() < end, "left running: {ids:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn reports_skill_status_where_install_skills_would_write() {
    let lab = Lab::new();
    let config = lab.path("claude-config");
    let installed = lab
        .cmd(&["install-skills", "--target", "claude", "--yes"])
        .env("CLAUDE_CONFIG_DIR", &config)
        .output()
        .unwrap();
    assert!(installed.status.success());
    lab.write(".agents/skills/corral/SKILL.md", "an older skill\n");
    let before = lab.files();
    let o = lab
        .cmd(&["integration", "status"])
        .env("CLAUDE_CONFIG_DIR", &config)
        .output()
        .unwrap();
    assert!(o.status.success());
    assert_eq!(lab.files(), before, "integration status wrote something");
    let s: Value = serde_json::from_slice(&o.stdout).unwrap();
    let omp = &s["agents"][3];
    assert_eq!(omp["kind"], "omp");
    assert_eq!(
        omp["skills"],
        json!([
            {"target":"claude","path":format!("{}/skills/corral/SKILL.md", config.display()),"status":"same"},
            {"target":"codex","path":lab.path(".agents/skills/corral/SKILL.md"),"status":"overwrite"},
        ])
    );
    assert_eq!(s["agents"][2]["skills"], json!([omp["skills"][1]]));

    // A skill directory that is a link belongs to someone else.
    fs::remove_dir_all(lab.path(".agents/skills")).unwrap();
    fs::create_dir_all(lab.path("elsewhere/corral")).unwrap();
    symlink(lab.path("elsewhere"), lab.path(".agents/skills")).unwrap();
    assert_eq!(lab.status()["agents"][1]["skills"][0]["status"], "foreign");
}

#[test]
fn rejects_unknown_integration_commands() {
    let lab = Lab::new();
    for args in [
        &["integration"][..],
        &["integration", "install"],
        &["integration", "status", "extra"],
        &["integration", "status", "--json"],
    ] {
        let o = lab.out(args);
        assert_eq!(o.status.code(), Some(1), "{args:?}");
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(v["error"], "usage", "{args:?}");
    }
}

const PLUGIN: &str = include_str!("../resources/amp_plugin.ts");

impl Lab {
    fn json(&self, args: &[&str]) -> (Option<i32>, Value) {
        let o = self
            .cmd(args)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        (o.status.code(), serde_json::from_slice(&o.stdout).unwrap())
    }
    fn amp(&self) -> Value {
        self.status()["agents"][4].clone()
    }
    fn agy(&self) -> Value {
        self.status()["agents"][5].clone()
    }
}

#[test]
fn installs_and_removes_only_the_amp_plugin() {
    let lab = Lab::new();
    let plugin = lab.path(".config/amp/plugins/corral.ts");
    // The user's own plugin and settings are never touched.
    lab.write(
        ".config/amp/plugins/mine.ts",
        "export default function () {}\n",
    );
    lab.write(".config/amp/settings.json", "{}\n");
    let untouched = lab.files();

    let (code, v) = lab.json(&["integration", "install", "--agent", "amp", "--dry-run"]);
    assert_eq!(code, Some(0));
    assert_eq!(
        v,
        json!({"ok":true,"action":"install","agent":"amp","path":plugin,"status":"create","dry_run":true,"written":false,"warnings":[]})
    );
    let (code, v) = lab.json(&["integration", "install", "--agent", "amp"]);
    assert_eq!(
        (code, &v["error"]),
        (Some(1), &json!("confirmation_required"))
    );
    assert_eq!(lab.files(), untouched, "wrote without confirmation");

    let (code, v) = lab.json(&["integration", "install", "--agent", "amp", "--yes"]);
    assert_eq!(
        (code, &v["status"], &v["written"]),
        (Some(0), &json!("create"), &json!(true))
    );
    assert_eq!(fs::read_to_string(&plugin).unwrap(), PLUGIN);
    let mut expected = untouched.clone();
    expected.insert(plugin.clone(), PLUGIN.as_bytes().to_vec());
    assert_eq!(lab.files(), expected, "wrote more than the plugin");
    assert_eq!(
        lab.amp()["install"],
        json!({"path":plugin,"status":"same","version":1})
    );
    let (_, v) = lab.json(&["integration", "install", "--agent", "amp", "--yes"]);
    assert_eq!(
        (&v["status"], &v["written"]),
        (&json!("same"), &json!(false))
    );

    // An older or edited copy of corral's plugin is updated in place.
    fs::write(&plugin, "const CORRAL_INTEGRATION = 0;\n").unwrap();
    assert_eq!(
        lab.amp()["install"],
        json!({"path":plugin,"status":"overwrite","version":0})
    );
    let (_, v) = lab.json(&["integration", "install", "--agent", "amp", "--yes"]);
    assert_eq!(
        (&v["status"], &v["written"]),
        (&json!("overwrite"), &json!(true))
    );
    assert_eq!(fs::read_to_string(&plugin).unwrap(), PLUGIN);

    let (_, v) = lab.json(&["integration", "uninstall", "--agent", "amp", "--dry-run"]);
    assert_eq!(
        (&v["action"], &v["status"], &v["written"]),
        (&json!("uninstall"), &json!("remove"), &json!(false))
    );
    let (code, v) = lab.json(&["integration", "uninstall", "--agent", "amp"]);
    assert_eq!(
        (code, &v["error"]),
        (Some(1), &json!("confirmation_required"))
    );
    let (_, v) = lab.json(&["integration", "uninstall", "--agent", "amp", "--yes"]);
    assert_eq!(
        (&v["status"], &v["written"]),
        (&json!("remove"), &json!(true))
    );
    assert_eq!(
        lab.files(),
        untouched,
        "uninstall left something behind or removed too much"
    );
    let (_, v) = lab.json(&["integration", "uninstall", "--agent", "amp", "--yes"]);
    assert_eq!(
        (&v["status"], &v["written"]),
        (&json!("absent"), &json!(false))
    );
    assert_eq!(lab.amp()["install"]["status"], "create");
}

#[test]
fn leaves_plugins_corral_does_not_own_in_place() {
    let lab = Lab::new();
    let plugin = lab.path(".config/amp/plugins/corral.ts");
    // A file of the same name without corral's version mark.
    lab.write(
        ".config/amp/plugins/corral.ts",
        "export default function () {}\n",
    );
    let before = lab.files();
    for args in [
        &["integration", "install", "--agent", "amp", "--yes"][..],
        &["integration", "uninstall", "--agent", "amp", "--yes"],
    ] {
        let (code, v) = lab.json(args);
        assert_eq!(
            (code, &v["status"], &v["written"]),
            (Some(0), &json!("foreign"), &json!(false)),
            "{args:?}"
        );
        assert_eq!(v["warnings"].as_array().unwrap().len(), 1);
    }
    assert_eq!(lab.files(), before);
    assert_eq!(
        lab.amp()["install"],
        json!({"path":plugin,"status":"foreign","version":null})
    );

    // A plugin directory that is a link belongs to someone else, even when it is empty.
    fs::remove_dir_all(lab.path(".config/amp/plugins")).unwrap();
    fs::create_dir_all(lab.path("dotfiles/plugins")).unwrap();
    symlink(
        lab.path("dotfiles/plugins"),
        lab.path(".config/amp/plugins"),
    )
    .unwrap();
    let before = lab.files();
    let (_, v) = lab.json(&["integration", "install", "--agent", "amp", "--yes"]);
    assert_eq!(
        (&v["status"], &v["written"]),
        (&json!("foreign"), &json!(false))
    );
    assert_eq!(lab.files(), before);

    // So is a directory where the file should be.
    fs::remove_file(lab.path(".config/amp/plugins")).unwrap();
    fs::create_dir_all(&plugin).unwrap();
    assert_eq!(lab.amp()["install"]["status"], "foreign");
}

#[test]
fn follows_xdg_config_home_when_it_is_absolute() {
    let lab = Lab::new();
    let xdg = lab.path("xdg");
    let o = lab
        .cmd(&["integration", "install", "--agent", "amp", "--yes"])
        .env("XDG_CONFIG_HOME", &xdg)
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["path"], json!(xdg.join("amp/plugins/corral.ts")));
    assert!(xdg.join("amp/plugins/corral.ts").is_file());
    let o = lab
        .cmd(&["integration", "install", "--agent", "amp", "--dry-run"])
        .env("XDG_CONFIG_HOME", "relative")
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["path"], json!(lab.path(".config/amp/plugins/corral.ts")));
}

#[test]
fn install_is_only_for_agents_that_need_it() {
    let lab = Lab::new();
    let before = lab.files();
    for args in [
        &["integration", "install", "--agent", "claude", "--yes"][..],
        &["integration", "uninstall", "--agent", "omp", "--yes"],
        &["integration", "install", "--agent", "other", "--yes"],
        &["integration", "install", "--yes"],
        &["integration", "install", "amp", "--yes"],
        &["integration", "install", "--agent", "amp", "--json"],
    ] {
        let (code, v) = lab.json(args);
        assert_eq!((code, &v["error"]), (Some(1), &json!("usage")), "{args:?}");
    }
    assert_eq!(lab.files(), before);
}

const AGY_HOOKS: &str = include_str!("../resources/agy_hooks.json");
const AGY_PLUGIN: &str = include_str!("../resources/agy_plugin.json");

/// agy (R8) loads a plugin directory, so corral installs a directory holding only its own files.
#[test]
fn installs_and_removes_only_the_agy_plugin_directory() {
    let lab = Lab::new();
    let dir = lab.path(".gemini/config/plugins/corral");
    let (hooks, plugin) = (dir.join("hooks.json"), dir.join("plugin.json"));
    // The user's own plugin and agy's settings are never touched.
    lab.write(".gemini/config/plugins/mine/plugin.json", "{}\n");
    lab.write(".gemini/config/config.json", "{\"plugins\":{}}\n");
    let untouched = lab.files();

    let (code, v) = lab.json(&["integration", "install", "--agent", "agy", "--dry-run"]);
    assert_eq!(code, Some(0));
    assert_eq!(
        v,
        json!({"ok":true,"action":"install","agent":"agy","path":dir,"status":"create","dry_run":true,"written":false,"warnings":[]})
    );
    let (code, v) = lab.json(&["integration", "install", "--agent", "agy"]);
    assert_eq!(
        (code, &v["error"]),
        (Some(1), &json!("confirmation_required"))
    );
    assert_eq!(lab.files(), untouched, "wrote without confirmation");

    let (_, v) = lab.json(&["integration", "install", "--agent", "agy", "--yes"]);
    assert_eq!(
        (&v["status"], &v["written"]),
        (&json!("create"), &json!(true))
    );
    let mut expected = untouched.clone();
    expected.insert(dir.clone(), vec![]);
    expected.insert(hooks.clone(), AGY_HOOKS.as_bytes().to_vec());
    expected.insert(plugin.clone(), AGY_PLUGIN.as_bytes().to_vec());
    assert_eq!(lab.files(), expected, "wrote more than the plugin");
    assert_eq!(
        lab.agy()["install"],
        json!({"path":dir,"status":"same","version":1,"enabled":true})
    );

    // Whether agy has it switched on comes from agy's own settings.
    lab.write(
        ".gemini/config/config.json",
        "{\"plugins\":{\"corral\":{\"enabled\":false}}}\n",
    );
    assert_eq!(lab.agy()["install"]["enabled"], false);
    lab.write(".gemini/config/config.json", "not json");
    assert_eq!(lab.agy()["install"]["enabled"], Value::Null);
    fs::remove_file(lab.path(".gemini/config/config.json")).unwrap();
    assert_eq!(lab.agy()["install"]["enabled"], true);
    lab.write(".gemini/config/config.json", "{\"plugins\":{}}\n");

    // An older copy, or one with a file missing, is brought up to date.
    fs::write(&hooks, "{} CORRAL_INTEGRATION = 0").unwrap();
    assert_eq!(
        (
            &lab.agy()["install"]["status"],
            &lab.agy()["install"]["version"]
        ),
        (&json!("overwrite"), &json!(0))
    );
    fs::remove_file(&plugin).unwrap();
    let (_, v) = lab.json(&["integration", "install", "--agent", "agy", "--yes"]);
    assert_eq!(
        (&v["status"], &v["written"]),
        (&json!("overwrite"), &json!(true))
    );
    assert_eq!(lab.files(), expected);
    // A temporary file an interrupted install left does not make the directory someone else's,
    // and uninstalling removes it with the rest.
    lab.write(".gemini/config/plugins/corral/.corral-x.tmp", "");
    assert_eq!(lab.agy()["install"]["status"], "same");

    let (code, v) = lab.json(&["integration", "uninstall", "--agent", "agy"]);
    assert_eq!(
        (code, &v["error"]),
        (Some(1), &json!("confirmation_required"))
    );
    let (_, v) = lab.json(&["integration", "uninstall", "--agent", "agy", "--yes"]);
    assert_eq!(
        (&v["status"], &v["written"]),
        (&json!("remove"), &json!(true))
    );
    assert_eq!(
        lab.files(),
        untouched,
        "uninstall left something or removed too much"
    );
    let (_, v) = lab.json(&["integration", "uninstall", "--agent", "agy", "--yes"]);
    assert_eq!(v["status"], "absent");
    // Only a leftover temporary file is still corral's to remove.
    lab.write(".gemini/config/plugins/corral/.corral-y.tmp", "");
    let (_, v) = lab.json(&["integration", "uninstall", "--agent", "agy", "--yes"]);
    assert_eq!(
        (&v["status"], &v["warnings"]),
        (&json!("remove"), &json!([]))
    );
    assert_eq!(lab.files(), untouched);
}

#[test]
fn leaves_a_plugin_directory_corral_does_not_own_in_place() {
    let lab = Lab::new();
    let dir = lab.path(".gemini/config/plugins/corral");
    let check = |what: &str| {
        let before = lab.files();
        for args in [
            &["integration", "install", "--agent", "agy", "--yes"][..],
            &["integration", "uninstall", "--agent", "agy", "--yes"],
        ] {
            let (code, v) = lab.json(args);
            assert_eq!(
                (code, &v["status"], &v["written"]),
                (Some(0), &json!("foreign"), &json!(false)),
                "{what} {args:?}"
            );
            assert_eq!(v["warnings"].as_array().unwrap().len(), 1, "{what}");
        }
        assert_eq!(lab.files(), before, "{what}");
        assert_eq!(lab.agy()["install"]["status"], "foreign", "{what}");
    };
    assert_eq!(
        lab.json(&["integration", "install", "--agent", "agy", "--yes"])
            .1["written"],
        true
    );
    // A file corral did not write, beside its own.
    lab.write(".gemini/config/plugins/corral/notes.md", "mine\n");
    check("an extra file");
    fs::remove_file(dir.join("notes.md")).unwrap();
    // Names like corral's temporary files that are not one.
    lab.write(".gemini/config/plugins/corral/.corral-notes", "");
    check("a file named like corral's");
    fs::remove_file(dir.join(".corral-notes")).unwrap();
    fs::create_dir(dir.join(".corral-x.tmp")).unwrap();
    check("a directory named like a temporary file");
    fs::remove_dir(dir.join(".corral-x.tmp")).unwrap();
    // One of corral's names without corral's version mark.
    fs::write(dir.join("plugin.json"), "{\"name\":\"corral\"}\n").unwrap();
    check("a file without the mark");
    fs::remove_dir_all(&dir).unwrap();
    // A file where the directory should be.
    lab.write(".gemini/config/plugins/corral", "CORRAL_INTEGRATION = 1\n");
    check("a file for the directory");
    fs::remove_file(&dir).unwrap();
    // A linked directory, even holding corral's own files.
    fs::create_dir_all(lab.path("dotfiles/corral")).unwrap();
    fs::write(lab.path("dotfiles/corral/hooks.json"), AGY_HOOKS).unwrap();
    symlink(lab.path("dotfiles/corral"), &dir).unwrap();
    check("a linked directory");
    fs::remove_file(&dir).unwrap();
    // A linked file inside it.
    fs::create_dir_all(&dir).unwrap();
    fs::write(lab.path("dotfiles/plugin.json"), AGY_PLUGIN).unwrap();
    symlink(lab.path("dotfiles/plugin.json"), dir.join("plugin.json")).unwrap();
    check("a linked file");
}
