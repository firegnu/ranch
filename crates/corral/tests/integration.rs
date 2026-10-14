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
