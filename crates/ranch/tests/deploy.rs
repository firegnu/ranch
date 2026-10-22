use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
};
use tempfile::TempDir;

const PROGRAMS: [&str; 4] = ["corral", "ranch", "fence", "pony"];

struct Home(TempDir);

impl Home {
    fn new() -> Self {
        Self(tempfile::tempdir().unwrap())
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.0.path().join(relative)
    }

    fn version(&self, name: &str) -> PathBuf {
        self.path(".local/share/ranch/versions").join(name)
    }

    fn package(&self, name: &str) {
        static SYNTHETIC: OnceLock<TempDir> = OnceLock::new();
        let synthetic = SYNTHETIC.get_or_init(|| {
            let dir = tempfile::tempdir().unwrap();
            let output = Command::new("rustc")
                .arg("--edition=2024")
                .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/deploy_program.rs"))
                .arg("-o")
                .arg(dir.path().join("program"))
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
            dir
        });
        let bin = self.version(name).join("bin");
        fs::create_dir_all(&bin).unwrap();
        let ranch = Path::new(env!("CARGO_BIN_EXE_ranch"));
        // Built by `cargo build -p corral-core --bin corral` for the focused test,
        // and by `cargo test --all-targets` for the full workspace run.
        let corral = ranch.parent().unwrap().join("corral");
        assert!(
            corral.is_file(),
            "build workspace corral first: {}",
            corral.display()
        );
        for (name, source) in [
            ("ranch", ranch.to_path_buf()),
            ("corral", corral),
            ("fence", synthetic.path().join("program")),
            ("pony", synthetic.path().join("program")),
        ] {
            fs::copy(source, bin.join(name)).unwrap();
        }
        self.manifest(name);
    }

    fn manifest(&self, name: &str) {
        let version = self.version(name);
        let mut text =
            format!("revision: {name}\ntarget: test-platform\nworking-tree: clean\nbranch: test\n");
        let mut entries: Vec<_> = fs::read_dir(version.join("bin"))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let hash = Sha256::digest(fs::read(entry.path()).unwrap());
            text.push_str(&format!(
                "{hash:x}  bin/{}\n",
                entry.file_name().to_str().unwrap()
            ));
        }
        fs::write(version.join("BUILD.txt"), text).unwrap();
    }

    fn link(&self, program: &str, version: &str) {
        fs::create_dir_all(self.path(".local/bin")).unwrap();
        let link = self.path(".local/bin").join(program);
        if fs::symlink_metadata(&link).is_ok() {
            fs::remove_file(&link).unwrap();
        }
        symlink(self.version(version).join("bin").join(program), link).unwrap();
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ranch"));
        command
            .env_clear()
            .env("HOME", self.0.path())
            .env("CORRAL_HOME", self.path(".corral"))
            .env("PONY_HOME", self.path(".pony"))
            .env(
                "DEPLOY_REAL_CORRAL",
                Path::new(env!("CARGO_BIN_EXE_ranch"))
                    .parent()
                    .unwrap()
                    .join("corral"),
            )
            .env("PATH", self.path(".local/bin"))
            .current_dir(self.0.path())
            .stdin(Stdio::null())
            .args(["deploy"])
            .args(args);
        command
    }

    fn run(&self, args: &[&str], code: i32) -> String {
        let output = self.command(args).output().unwrap();
        let text = String::from_utf8(output.stdout).unwrap();
        assert_eq!(
            output.status.code(),
            Some(code),
            "{text}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        text
    }

    fn records(&self) -> Vec<Value> {
        fs::read_to_string(self.path(".local/share/ranch/deploys.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn snapshot(&self) -> BTreeMap<PathBuf, String> {
        fn visit(path: &Path, tree: &mut BTreeMap<PathBuf, String>) {
            for entry in fs::read_dir(path).unwrap() {
                let path = entry.unwrap().path();
                let metadata = fs::symlink_metadata(&path).unwrap();
                let value = if metadata.is_symlink() {
                    format!("link:{}", fs::read_link(&path).unwrap().display())
                } else if metadata.is_dir() {
                    visit(&path, tree);
                    "directory".into()
                } else {
                    format!("{:x}", Sha256::digest(fs::read(&path).unwrap()))
                };
                tree.insert(path, value);
            }
        }
        let mut tree = BTreeMap::new();
        visit(self.0.path(), &mut tree);
        tree
    }
}

#[test]
fn dry_run_prints_plan_without_changing_anything_even_with_yes() {
    let home = Home::new();
    home.package("new");
    let before = home.snapshot();
    let out = home.run(&["new", "--dry-run", "--yes"], 0);
    assert!(out.contains("要做的："), "{out}");
    assert!(out.contains("不做的："), "{out}");
    assert!(out.contains("退回"), "{out}");
    assert!(out.contains("corral is not on PATH"), "{out}");
    assert_eq!(home.snapshot(), before);
}

#[test]
fn deploy_switches_all_links_installs_changes_records_rollback_and_then_is_noop() {
    let home = Home::new();
    home.package("old");
    home.package("new");
    for name in PROGRAMS {
        home.link(name, "old");
    }
    let skill = home.path(".agents/skills/corral/SKILL.md");
    fs::create_dir_all(skill.parent().unwrap()).unwrap();
    fs::write(&skill, "old skill").unwrap();
    let plugin = home.path(".config/amp/plugins/corral.ts");
    fs::create_dir_all(plugin.parent().unwrap()).unwrap();
    fs::write(&plugin, "// CORRAL_INTEGRATION = 1\nold plugin").unwrap();
    let foreign = home.path("custom-skills");
    fs::create_dir_all(&foreign).unwrap();
    fs::write(foreign.join("SKILL.md"), "user skill").unwrap();
    fs::create_dir_all(home.path(".claude/skills")).unwrap();
    symlink(&foreign, home.path(".claude/skills/corral")).unwrap();
    symlink(&foreign, home.path(".claude/skills/corral-dispatch")).unwrap();
    let out = home.run(&["new", "--yes"], 0);
    assert!(out.contains("程序相同"), "{out}");
    assert!(out.contains("deploy old"), "{out}");
    assert!(out.contains("foreign"), "{out}");
    assert_eq!(
        fs::read_to_string(foreign.join("SKILL.md")).unwrap(),
        "user skill"
    );
    for name in PROGRAMS {
        assert_eq!(
            fs::read_link(home.path(".local/bin").join(name)).unwrap(),
            fs::canonicalize(home.version("new").join("bin").join(name)).unwrap()
        );
    }
    assert_ne!(fs::read_to_string(&skill).unwrap(), "old skill");
    assert_ne!(
        fs::read_to_string(&plugin).unwrap(),
        "// CORRAL_INTEGRATION = 1\nold plugin"
    );
    assert!(!home.path(".gemini/config/plugins/corral").exists());
    let records = home.records();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0]["result"], "started");
    assert_eq!(records[0]["before_version"], "old");
    assert_eq!(records[1]["result"], "done");
    assert!(
        records[1]["rollback"]
            .as_str()
            .unwrap()
            .ends_with(" deploy old")
    );
    assert_eq!(records[1]["after"]["pony"], "new");
    let before = home.snapshot();
    let modified = fs::metadata(&skill).unwrap().modified().unwrap();
    let again = home.run(&["new"], 0);
    assert!(again.contains("已经是 new，没有要做的"), "{again}");
    assert_eq!(home.snapshot(), before);
    assert_eq!(fs::metadata(&skill).unwrap().modified().unwrap(), modified);
    // The printed rollback command uses this same deploy implementation. Neither
    // installer is needed when moving links back to byte-identical programs.
    home.run(&["old", "--yes"], 0);
    assert_eq!(fs::metadata(&skill).unwrap().modified().unwrap(), modified);
    let records = home.records();
    assert_eq!(records.len(), 4);
    assert_eq!(records[3]["steps"].as_array().unwrap().len(), 2);
    assert_eq!(records[3]["after"]["corral"], "old");
}

#[test]
fn nonterminal_without_yes_prints_plan_but_changes_nothing() {
    let home = Home::new();
    home.package("new");
    let before = home.snapshot();
    let out = home.run(&["new"], 1);
    assert!(out.contains("要做的："), "{out}");
    assert!(out.contains("confirmation_required"), "{out}");
    assert_eq!(home.snapshot(), before);
    // First installation creates the bin directory and links only after consent.
    let out = home.run(&["new", "--yes"], 0);
    assert!(out.contains("rm -- "), "{out}");
    for name in PROGRAMS {
        assert_eq!(
            fs::read_link(home.path(".local/bin").join(name)).unwrap(),
            fs::canonicalize(home.version("new").join("bin").join(name)).unwrap()
        );
    }
    assert_eq!(home.records()[0]["before"]["corral"], Value::Null);
    assert_eq!(home.records()[1]["result"], "done");
}

#[test]
fn refuses_mismatched_checksum_regular_destination_and_unknown_program() {
    let home = Home::new();
    home.package("new");
    let manifest = home.version("new").join("BUILD.txt");
    let original = fs::read_to_string(&manifest).unwrap();
    let text = original
        .lines()
        .map(|line| {
            if line.ends_with("  bin/pony") {
                format!("{}  bin/pony", "0".repeat(64))
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&manifest, text).unwrap();
    let before = home.snapshot();
    assert!(home.run(&["new", "--yes"], 1).contains("校验和"));
    assert_eq!(home.snapshot(), before);

    fs::write(manifest, original).unwrap();
    fs::create_dir_all(home.path(".local/bin")).unwrap();
    fs::write(home.path(".local/bin/corral"), "user program").unwrap();
    let before = home.snapshot();
    assert!(home.run(&["new", "--yes"], 1).contains("不是符号链接"));
    assert_eq!(home.snapshot(), before);

    fs::remove_file(home.path(".local/bin/corral")).unwrap();
    fs::copy(
        home.version("new").join("bin/pony"),
        home.version("new").join("bin/future"),
    )
    .unwrap();
    home.manifest("new");
    let before = home.snapshot();
    let out = home.run(&["new", "--yes"], 1);
    assert!(out.contains("不认识的程序 future"), "{out}");
    assert!(out.contains("/new/bin/ranch deploy new"), "{out}");
    assert_eq!(home.snapshot(), before);
}

#[test]
fn mixed_links_find_previous_version_by_bytes_or_print_individual_rollback() {
    let home = Home::new();
    for version in ["a", "b", "new"] {
        home.package(version);
    }
    // a and b share three programs, but only b has the currently linked pony.
    fs::copy("/bin/cat", home.version("a").join("bin/pony")).unwrap();
    home.manifest("a");
    for name in ["corral", "ranch", "fence"] {
        home.link(name, "a");
    }
    home.link("pony", "b");
    let out = home.run(&["new", "--yes"], 0);
    assert!(out.contains("deploy b"), "{out}");
    assert_eq!(home.records()[0]["before_version"], "b");
    assert_eq!(home.records()[0]["before"]["corral"], "a");
    assert_eq!(home.records()[0]["before"]["pony"], "b");

    // Neither candidate now contains all four currently linked byte sequences.
    for name in ["corral", "ranch", "fence"] {
        home.link(name, "a");
    }
    home.link("pony", "b");
    fs::copy("/bin/cat", home.version("b").join("bin/fence")).unwrap();
    home.manifest("b");
    let before = home.snapshot();
    let out = home.run(&["new", "--dry-run"], 0);
    assert!(out.contains("无法归到一个旧版本"), "{out}");
    assert!(out.contains("ln -sfn -- "), "{out}");
    assert!(out.contains("/b/bin/pony"), "{out}");
    assert!(out.contains("/a/bin/corral"), "{out}");
    assert_eq!(home.snapshot(), before);
}

#[test]
fn failed_install_stops_records_state_and_retry_finishes_remaining_steps() {
    let home = Home::new();
    home.package("old");
    home.package("new");
    // A Rust wrapper forwards read-only checks and successful installs to the
    // workspace corral, but can fail its install without touching real user state.
    fs::copy(
        home.version("new").join("bin/pony"),
        home.version("new").join("bin/corral"),
    )
    .unwrap();
    home.manifest("new");
    for name in PROGRAMS {
        home.link(name, "old");
    }
    let helper = home.path(".corral/.runtime/helper");
    fs::create_dir_all(helper.parent().unwrap()).unwrap();
    symlink(home.version("old").join("bin/corral"), &helper).unwrap();
    fs::write(home.path(".corral/user-agent-sentinel"), "untouched").unwrap();
    fs::write(home.path("fail-install"), "fail").unwrap();
    let out = home.run(&["new", "--yes"], 1);
    assert!(out.contains("synthetic install failure"), "{out}");
    assert!(out.contains("不自动撤回"), "{out}");
    assert!(out.contains("没做 ranch 技能"), "{out}");
    assert!(out.contains("当前链接 corral"), "{out}");
    assert!(out.contains("deploy old"), "{out}");
    assert!(!home.path(".agents/skills/corral-dispatch").exists());
    let records = home.records();
    assert_eq!(records.len(), 2);
    assert_eq!(records[1]["result"], "failed");
    assert_eq!(records[1]["after"]["corral"], "new");
    assert_eq!(records[1]["steps"][0]["result"], "done");
    assert_eq!(records[1]["steps"][1]["result"], "failed");
    assert_eq!(records[1]["steps"][2]["result"], "not_done");
    fs::remove_file(home.path("fail-install")).unwrap();
    let out = home.run(&["new", "--yes"], 0);
    assert!(out.contains("上次部署："), "{out}");
    assert!(out.contains("，失败"), "{out}");
    assert!(!out.contains("切链接（"), "{out}");
    assert!(
        home.path(".agents/skills/corral-dispatch/SKILL.md")
            .is_file()
    );
    assert_eq!(home.records().len(), 4);
    assert_eq!(home.records()[3]["result"], "done");
    assert!(
        out.lines()
            .any(|line| line.starts_with("退回：") && line.ends_with(" deploy old")),
        "{out}"
    );
    assert!(
        home.records()[3]["rollback"]
            .as_str()
            .unwrap()
            .ends_with(" deploy old")
    );
    assert_eq!(home.records()[3]["before_version"], "old");
    assert!(out.contains("接着上次未完成的部署，退回沿用那次"), "{out}");
    let again = home.run(&["new", "--dry-run"], 0);
    assert!(
        again
            .lines()
            .any(|line| line.starts_with("做完后退回：") && line.ends_with(" deploy old")),
        "{again}"
    );
    assert_eq!(
        fs::read_link(helper).unwrap(),
        home.version("old").join("bin/corral")
    );
    assert_eq!(
        fs::read_to_string(home.path(".corral/user-agent-sentinel")).unwrap(),
        "untouched"
    );
    assert!(!home.path(".pony").exists());
}

#[test]
fn rework_interrupted_partial_links_keep_the_original_individual_rollback() {
    let home = Home::new();
    home.package("old");
    home.package("new");
    for name in PROGRAMS {
        home.link(name, "old");
    }
    // An external pony makes a single before_version impossible.
    let original_pony = home.path("original-pony");
    fs::copy("/bin/cat", &original_pony).unwrap();
    fs::remove_file(home.path(".local/bin/pony")).unwrap();
    symlink(&original_pony, home.path(".local/bin/pony")).unwrap();
    let preview = home.run(&["new", "--dry-run"], 0);
    let rollback = preview
        .split_once("做完后退回：")
        .unwrap()
        .1
        .trim()
        .to_owned();
    let record = serde_json::json!({"to":"new", "before_version":null, "rollback":rollback, "result":"started"});
    fs::write(
        home.path(".local/share/ranch/deploys.jsonl"),
        format!("{record}\n"),
    )
    .unwrap();
    // Simulate interruption after the first atomic switch; no process is killed.
    home.link("corral", "new");
    let out = home.run(&["new", "--yes"], 0);
    assert!(out.contains("接着上次未完成的部署，退回沿用那次"), "{out}");
    assert!(out.contains("没有结束记录"), "{out}");
    assert!(out.contains(&rollback), "{out}");
    assert!(rollback.contains("/old/bin/corral"), "{rollback}");
    let records = home.records();
    assert_eq!(records.len(), 3);
    assert_eq!(records[2]["result"], "done");
    assert_eq!(records[2]["rollback"], rollback);
    assert_eq!(records[2]["before_version"], Value::Null);
}

#[test]
fn rework_integration_status_can_take_more_than_five_seconds() {
    let home = Home::new();
    home.package("new");
    fs::copy(
        home.version("new").join("bin/fence"),
        home.version("new").join("bin/corral"),
    )
    .unwrap();
    home.manifest("new");
    let output = home
        .command(&["new", "--dry-run"])
        .env("DEPLOY_SLOW_STATUS", "1")
        .output()
        .unwrap();
    let out = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "{out}");
    assert!(out.contains("全局插件 amp"), "{out}");
    assert!(!home.path(".local/share/ranch/deploys.jsonl").exists());
}

#[test]
fn rework_timeout_does_not_signal_the_group_after_its_leader_was_reaped() {
    let home = Home::new();
    home.package("new");
    let finished = home.path("pipe-holder-finished");
    let output = home
        .command(&["new", "--dry-run"])
        .env("DEPLOY_HELD_PIPE_DONE", &finished)
        .output()
        .unwrap();
    let out = String::from_utf8(output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(1), "{out}");
    assert!(out.contains("超过 5 秒"), "{out}");
    // Wait for this finite-lived synthetic descendant, never a real agent.
    // It finishes after the timeout only if the reaped leader's group was not signalled.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
    while !finished.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        finished.exists(),
        "the already-reaped leader's group was killed: {out}"
    );
}

#[test]
fn rework_pony_notice_distinguishes_first_install_and_platform() {
    let home = Home::new();
    home.package("new");
    let out = home.run(&["new", "--dry-run"], 0);
    assert!(
        out.contains("pony 第一次装，服务另装，deploy 不装"),
        "{out}"
    );
    assert!(!out.contains("launchctl kickstart"), "{out}");

    home.package("old");
    fs::copy("/bin/cat", home.version("old").join("bin/pony")).unwrap();
    home.manifest("old");
    home.link("pony", "old");
    let out = home.run(&["new", "--dry-run"], 0);
    assert!(!out.contains("pony 第一次装"), "{out}");
    assert_eq!(
        out.contains("要你手动重启 pony：launchctl kickstart"),
        cfg!(target_os = "macos"),
        "{out}"
    );
    assert!(!home.path(".pony").exists());
}

#[test]
fn rework_manifest_program_missing_from_bin_is_rejected() {
    let home = Home::new();
    home.package("new");
    fs::remove_file(home.version("new").join("bin/pony")).unwrap();
    let before = home.snapshot();
    let out = home.run(&["new", "--dry-run"], 1);
    assert!(out.contains("BUILD.txt 列出的 bin/pony 缺失"), "{out}");
    assert_eq!(home.snapshot(), before);
    // Old packages that never declared pony remain valid.
    home.manifest("new");
    let out = home.run(&["new", "--dry-run"], 0);
    assert!(out.contains("目标版本没有，不动这个链接"), "{out}");
}
