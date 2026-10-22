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

#[allow(dead_code)]
#[path = "../src/deploy.rs"]
mod deploy;

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
        // Service cases must use pony_run: a native launchctl target does not follow HOME.
        assert!(
            fs::symlink_metadata(self.path("Library/LaunchAgents/local.ranch.pony.plist"))
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
            "pony service cases must use pony_run"
        );
        let mut command = Command::new(env!("CARGO_BIN_EXE_ranch"));
        self.environment(&mut command);
        command.args(["deploy"]).args(args);
        command
    }

    fn environment(&self, command: &mut Command) {
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
            .stdin(Stdio::null());
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

// A child test process provides fake OS calls to the real deploy command entry.
// No environment hooks or fake launchctl paths exist in the production binary.
#[test]
fn pony_cli_fixture() {
    let Ok(args) = std::env::var("DEPLOY_PONY_ARGS") else {
        return;
    };
    let system = deploy::pony::System {
        macos: std::env::var_os("DEPLOY_NOT_MACOS").is_none(),
        process: |pid| {
            let home = PathBuf::from(std::env::var_os("HOME").unwrap());
            let after = home.join(format!("process-after-{pid}.json"));
            let path = if home.join("pony-kickstarted").exists() && after.exists() {
                after
            } else {
                home.join(format!("process-{pid}.json"))
            };
            let data: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
            Ok(deploy::pony::Process {
                parent: data["parent"].as_u64().unwrap() as u32,
                program: PathBuf::from(data["program"].as_str().unwrap()),
            })
        },
        launch: |args| {
            assert_eq!(
                args,
                [
                    "kickstart",
                    "-k",
                    &format!("gui/{}/local.ranch.pony", unsafe { libc::geteuid() })
                ]
            );
            let home = PathBuf::from(std::env::var_os("HOME").unwrap());
            // Restart must follow the link switch.
            assert!(
                fs::read_link(home.join(".local/bin/pony"))
                    .unwrap()
                    .ends_with("new/bin/pony")
            );
            fs::rename(
                home.join("pony-status-next.json"),
                home.join("pony-status.json"),
            )
            .unwrap();
            fs::write(home.join("pony-kickstarted"), "yes").unwrap();
            Ok(())
        },
    };
    std::process::exit(
        deploy::run_with(&args.split_whitespace().collect::<Vec<_>>(), &system).into(),
    );
}

impl Home {
    fn pony_run(&self, args: &str, code: i32, macos: bool) -> String {
        let mut command = Command::new(std::env::current_exe().unwrap());
        self.environment(&mut command);
        command
            .args(["--exact", "pony_cli_fixture", "--nocapture"])
            .env("DEPLOY_PONY_ARGS", args);
        if !macos {
            command.env("DEPLOY_NOT_MACOS", "1");
        }
        let output = command.output().unwrap();
        let out = String::from_utf8(output.stdout).unwrap();
        assert_eq!(
            output.status.code(),
            Some(code),
            "{out}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        out
    }

    fn pony_status(&self, file: &str, service: &str, pid: Option<i32>) {
        fs::write(self.path(file), serde_json::to_vec(&serde_json::json!({
            "ok":true, "service":service, "running":pid.is_some(), "pid":pid,
            "last_exit": if pid.is_none() { serde_json::json!({"code":"config_missing"}) } else { Value::Null },
            "chats":[{"kind":"slack","connection":"connecting"}]
        })).unwrap()).unwrap();
    }

    fn pony_process(&self, pid: i32, parent: u32, program: &Path) {
        fs::write(
            self.path(&format!("process-{pid}.json")),
            serde_json::to_vec(&serde_json::json!({"parent":parent,"program":program})).unwrap(),
        )
        .unwrap();
    }

    fn running_pony() -> Self {
        let home = Self::new();
        home.package("old");
        home.package("new");
        // Different bytes, no actual running process.
        fs::copy("/bin/cat", home.version("old").join("bin/pony")).unwrap();
        home.manifest("old");
        for name in PROGRAMS {
            home.link(name, "old");
        }
        home.pony_status("pony-status.json", "installed", Some(101));
        home.pony_process(101, 1, &home.version("old").join("bin/pony"));
        home.pony_status("pony-status-next.json", "installed", Some(102));
        home.pony_process(102, 1, &home.version("new").join("bin/pony"));
        home
    }
}

#[test]
fn pony_changed_running_program_is_restarted_and_verified() {
    let home = Home::running_pony();
    // Neither the same pid with target bytes, a new pid with old bytes, nor
    // running:false may satisfy the restart check.
    fs::copy(
        home.path("process-102.json"),
        home.path("process-after-101.json"),
    )
    .unwrap();
    home.pony_process(103, 1, &home.version("old").join("bin/pony"));
    home.pony_status("pony-poll-0.json", "installed", Some(101));
    home.pony_status("pony-poll-1.json", "installed", Some(103));
    home.pony_status("pony-poll-2.json", "installed", None);
    let before = home.snapshot();
    let out = home.pony_run("new --dry-run", 0, true);
    assert!(out.contains("重启 pony：launchctl kickstart"), "{out}");
    assert_eq!(home.snapshot(), before);
    let out = home.pony_run("new --yes", 0, true);
    assert!(out.contains("pid 102"), "{out}");
    assert!(out.contains("slack connecting"), "{out}");
    assert!(out.contains("pony 跑的是目标程序"), "{out}");
    assert!(home.path("pony-kickstarted").exists());
    for n in 0..3 {
        assert!(!home.path(&format!("pony-poll-{n}.json")).exists());
    }
    let records = home.records();
    assert_eq!(records[0]["pony"]["action"], "pony_restart");
    let step = records[1]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["step"] == "pony_restart")
        .unwrap();
    assert_eq!(step["result"], "done");
    assert_eq!(step["pid"], 102);
    assert!(!home.path(".pony").exists());
}

#[test]
fn pony_old_target_without_service_is_refused_when_plist_exists() {
    let home = Home::running_pony();
    // R38f predecessors always said not_installed, even with a newer plist present.
    home.pony_status("pony-status.json", "not_installed", Some(101));
    fs::write(home.path("pony-no-service"), "unsupported").unwrap();
    let plist = home.path("Library/LaunchAgents/local.ranch.pony.plist");
    fs::create_dir_all(plist.parent().unwrap()).unwrap();
    fs::write(plist, "installed service").unwrap();
    let before = home.snapshot();
    let out = home.pony_run("new --yes", 1, true);
    assert!(out.contains("先用当前版本卸服务再部署"), "{out}");
    assert_eq!(home.snapshot(), before);
}

#[test]
fn pony_changed_plist_uses_target_service_install_without_kickstart() {
    let home = Home::running_pony();
    fs::write(home.path("pony-replace"), "replace").unwrap();
    let before = home.snapshot();
    let out = home.pony_run("new --dry-run", 0, true);
    assert!(out.contains("pony service install --yes"), "{out}");
    assert_eq!(home.snapshot(), before);
    let out = home.pony_run("new --yes", 0, true);
    assert!(out.contains("pid 102"), "{out}");
    assert!(home.path("pony-installed").exists());
    assert!(!home.path("pony-kickstarted").exists());
    let records = home.records();
    let step = records[1]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["step"] == "pony_service")
        .unwrap();
    assert_eq!(step["result"], "done");
    assert_eq!(step["pid"], 102);
}

fn unloaded_pony_service() -> Home {
    let home = Home::running_pony();
    fs::write(home.path("pony-replace"), "replace").unwrap();
    fs::write(home.path("pony-service-fails"), "bootstrap failure").unwrap();
    home.pony_status("pony-status-unloaded.json", "installed", None);
    home
}

#[test]
fn rework_pony_service_failure_reports_unloaded_service_and_install_recovery() {
    let home = unloaded_pony_service();
    let out = home.pony_run("new --yes", 1, true);
    assert!(out.contains("pony 现在没在跑"), "{out}");
    assert!(
        out.contains("服务定义可能已卸下，launchd 不会自己再起 pony"),
        "{out}"
    );
    assert!(!out.contains("launchd 会按服务策略再试"), "{out}");
    let recovery = out
        .lines()
        .find(|line| line.starts_with("接着做："))
        .unwrap();
    assert!(
        recovery.contains(&format!(
            "{} service install --yes",
            home.version("new").join("bin/pony").display()
        )),
        "{recovery}"
    );
    assert!(!recovery.contains(" deploy new"), "{recovery}");
    assert!(out.contains(".pony/logs/launchd.log"), "{out}");
    assert_eq!(home.records()[1]["pony"]["before"]["running"], true);
    assert_eq!(home.records()[1]["pony_after"]["running"], false);
}

#[test]
fn rework_pony_service_retry_restores_a_service_stopped_by_deploy() {
    let home = unloaded_pony_service();
    home.pony_run("new --yes", 1, true);
    for attempt in 0..2 {
        let before = home.snapshot();
        let out = home.pony_run("new --dry-run", 0, true);
        assert!(out.contains("pony service install --yes"), "{out}");
        assert!(!out.contains("没有要做的"), "{out}");
        assert_eq!(home.snapshot(), before);
        if attempt == 0 {
            home.pony_run("new --yes", 1, true);
            assert_eq!(home.records()[3]["pony"]["before"]["running"], true);
        }
    }
    fs::remove_file(home.path("pony-service-fails")).unwrap();
    let out = home.pony_run("new --yes", 0, true);
    assert!(out.contains("pid 102"), "{out}");
    assert!(out.contains("pony 跑的是目标程序"), "{out}");
    assert!(!out.contains("没有要做的"), "{out}");
    assert!(home.path("pony-installed").exists());
    assert!(!home.path("pony-kickstarted").exists());
    let records = home.records();
    assert_eq!(records[5]["result"], "done");
    assert_eq!(records[5]["before_version"], "old");
    let step = records[5]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["step"] == "pony_service")
        .unwrap();
    assert_eq!(step["result"], "done");
    assert_eq!(step["pid"], 102);
}

#[test]
fn pony_stopped_service_is_not_started_even_if_plist_needs_replacing() {
    let home = Home::running_pony();
    home.pony_status("pony-status.json", "installed", None);
    fs::write(home.path("pony-replace"), "replace").unwrap();
    let out = home.pony_run("new --yes", 0, true);
    assert!(out.contains("服务装着但没在跑，不重启"), "{out}");
    assert!(out.contains("config_missing"), "{out}");
    assert!(
        out.contains("服务定义要换（replace），deploy 没换"),
        "{out}"
    );
    assert!(
        out.contains("pony service install --yes（会启动 pony）"),
        "{out}"
    );
    assert!(home.path("pony-replace").exists());
    assert!(!home.path("pony-kickstarted").exists());
    assert!(!home.path("pony-installed").exists());
    assert_eq!(home.records()[1]["pony"]["action"], "skip");
}

#[test]
fn pony_manual_process_is_left_running_even_if_plist_needs_replacing() {
    let home = Home::running_pony();
    home.pony_process(101, 88, &home.version("old").join("bin/pony"));
    fs::write(home.path("pony-replace"), "replace").unwrap();
    let out = home.pony_run("new --yes", 0, true);
    assert!(
        out.contains("父进程不是 launchd，不重启，请自行重启"),
        "{out}"
    );
    assert!(
        out.contains("服务定义要换（replace），deploy 没换"),
        "{out}"
    );
    assert!(out.contains("先停掉手动起的 pony"), "{out}");
    assert!(
        out.contains("pony service install --yes（会启动 pony）"),
        "{out}"
    );
    assert!(home.path("pony-replace").exists());
    assert!(!home.path("pony-kickstarted").exists());
    assert!(!home.path("pony-installed").exists());
    assert_eq!(home.records()[1]["pony"]["action"], "skip");
}

#[test]
fn pony_identical_running_program_at_another_path_is_not_restarted() {
    let home = Home::running_pony();
    let running = home.path("running-pony");
    fs::copy(home.version("new").join("bin/pony"), &running).unwrap();
    home.pony_process(101, 1, &running);
    let out = home.pony_run("new --yes", 0, true);
    assert!(out.contains("正在跑的 pony 程序相同，不重启"), "{out}");
    assert!(out.contains("pony 跑的是目标程序（pid 101"), "{out}");
    assert!(!home.path("pony-kickstarted").exists());
    assert!(!home.path("pony-installed").exists());
}

#[test]
fn rework_pony_restart_plan_names_running_version_or_external_program() {
    let home = Home::running_pony();
    let out = home.pony_run("new --dry-run", 0, true);
    assert!(
        out.contains("正在跑的是 old 的 pony（pid 101），换成 new"),
        "{out}"
    );
    let external = home.path("external-pony");
    fs::copy(home.version("old").join("bin/pony"), &external).unwrap();
    home.pony_process(101, 1, &external);
    let out = home.pony_run("new --dry-run", 0, true);
    assert!(
        out.contains(&format!(
            "正在跑的是 {} 的 pony（pid 101），换成 new",
            external.display()
        )),
        "{out}"
    );
}

#[test]
fn pony_unsupported_platform_only_switches_links() {
    for macos in [false, true] {
        let home = Home::running_pony();
        home.pony_status("pony-status.json", "unsupported", Some(101));
        let out = home.pony_run("new --yes", 0, macos);
        assert!(out.contains("跳过重启"), "{out}");
        assert!(!home.path("pony-kickstarted").exists());
        assert!(!home.path("pony-installed").exists());
        assert_eq!(home.records()[1]["after"]["pony"], "new");
    }
}

#[test]
fn pony_retry_still_restarts_old_process_after_links_already_changed() {
    let home = Home::running_pony();
    fs::copy(
        home.version("new").join("bin/fence"),
        home.version("new").join("bin/corral"),
    )
    .unwrap();
    home.manifest("new");
    fs::write(home.path("fail-install"), "fail").unwrap();
    let out = home.pony_run("new --yes", 1, true);
    assert!(out.contains("没做 重启 pony"), "{out}");
    assert!(!out.contains("pony 现在"), "{out}");
    assert!(!out.contains(".pony/logs/"), "{out}");
    assert!(!home.path("pony-kickstarted").exists());
    fs::remove_file(home.path("fail-install")).unwrap();
    let before = home.snapshot();
    let out = home.pony_run("new --dry-run", 0, true);
    assert!(out.contains("接着上次未完成的部署"), "{out}");
    assert!(out.contains("重启 pony：launchctl kickstart"), "{out}");
    assert!(
        out.contains("正在跑的是 old 的 pony（pid 101），换成 new"),
        "{out}"
    );
    assert!(!out.contains("切链接（"), "{out}");
    assert_eq!(home.snapshot(), before);
    let out = home.pony_run("new --yes", 0, true);
    assert!(out.contains("pid 102"), "{out}");
    assert!(home.path("pony-kickstarted").exists());
    assert_eq!(home.records()[3]["before_version"], "old");
    assert_eq!(home.records()[3]["result"], "done");
}

#[test]
fn pony_restart_timeout_reports_stopped_state_first_and_records_failure() {
    let home = Home::running_pony();
    home.pony_status("pony-status-next.json", "installed", None);
    let started = std::time::Instant::now();
    let out = home.pony_run("new --yes", 1, true);
    assert!(started.elapsed() >= std::time::Duration::from_secs(30));
    assert!(out.contains("30 秒内没等到新的 pony 起来"), "{out}");
    assert!(out.contains("config_missing"), "{out}");
    assert!(!out.contains("launchd 会按服务策略再试"), "{out}");
    assert!(
        out.find("pony 现在没在跑").unwrap() < out.find("中途失败").unwrap(),
        "{out}"
    );
    assert!(out.contains(".pony/logs/launchd.log"), "{out}");
    assert!(out.contains(".pony/logs/pony.log"), "{out}");
    assert!(out.contains("没做 核对"), "{out}");
    assert!(out.contains("接着做："), "{out}");
    assert!(out.contains("退回："), "{out}");
    let records = home.records();
    assert_eq!(records[1]["result"], "failed");
    assert_eq!(records[1]["pony_after"]["running"], false);
    let step = records[1]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["step"] == "pony_restart")
        .unwrap();
    assert_eq!(step["result"], "failed");
    assert_eq!(records[1]["after"]["pony"], "new");
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
fn pony_not_installed_only_switches_links_even_when_program_changes() {
    let home = Home::new();
    home.package("new");
    let out = home.run(&["new", "--dry-run"], 0);
    assert!(
        out.contains(if cfg!(target_os = "macos") {
            "pony 服务没装，服务另装，deploy 不装"
        } else {
            "pony 非 macOS 平台，跳过重启"
        }),
        "{out}"
    );
    assert!(!out.contains("launchctl kickstart"), "{out}");

    home.package("old");
    fs::copy("/bin/cat", home.version("old").join("bin/pony")).unwrap();
    home.manifest("old");
    home.link("pony", "old");
    let out = home.run(&["new", "--dry-run"], 0);
    assert!(!out.contains("launchctl kickstart"), "{out}");
    let out = home.run(&["new", "--yes"], 0);
    assert!(!out.contains("launchctl kickstart"), "{out}");
    assert_eq!(home.records()[1]["pony"]["action"], "skip");
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

#[test]
fn history_truncated_in_a_chinese_character_still_allows_preview() {
    let home = Home::new();
    home.package("new");
    let mut history = b"{\"to\":\"old\",\"result\":\"done\"}\n{\"title\":\"".to_vec();
    history.extend_from_slice(&"切".as_bytes()[..2]);
    fs::write(home.path(".local/share/ranch/deploys.jsonl"), history).unwrap();
    let before = home.snapshot();
    let out = home.run(&["new", "--dry-run"], 0);
    assert!(out.contains("末行不完整（可能被中断）"), "{out}");
    assert_eq!(home.snapshot(), before);
}

#[test]
fn rework_history_append_separates_truncated_tail_from_new_records() {
    let home = Home::new();
    home.package("new");
    let mut history = b"{\"to\":\"old\",\"result\":\"done\"}\n{\"title\":\"".to_vec();
    history.extend_from_slice(&"切".as_bytes()[..2]);
    let path = home.path(".local/share/ranch/deploys.jsonl");
    fs::write(&path, &history).unwrap();
    home.run(&["new", "--yes"], 0);
    let appended = fs::read(path).unwrap();
    assert!(appended.starts_with(&history));
    let lines: Vec<_> = appended
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .collect();
    assert_eq!(
        lines.len(),
        4,
        "keep the old record and truncated tail, plus separate started/done records"
    );
    assert!(serde_json::from_slice::<Value>(lines[1]).is_err());
    assert_eq!(
        serde_json::from_slice::<Value>(lines[2]).unwrap()["result"],
        "started"
    );
    assert_eq!(
        serde_json::from_slice::<Value>(lines[3]).unwrap()["result"],
        "done"
    );
}
