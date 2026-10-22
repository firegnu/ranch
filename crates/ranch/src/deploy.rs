//! Deployment orchestration; all runtime-specific work goes through target-version commands.
#[path = "deploy/command.rs"]
mod command;
#[path = "deploy/pony.rs"]
pub(crate) mod pony;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    ffi::CString,
    fs,
    io::{self, IsTerminal, Read, Write},
    os::unix::fs::{OpenOptionsExt, symlink},
    path::{Path, PathBuf},
    time::Duration,
};

const PROGRAMS: [&str; 4] = ["corral", "ranch", "fence", "pony"];
type Result<T> = std::result::Result<T, String>;

struct Paths {
    bin: PathBuf,
    state: PathBuf,
    versions: PathBuf,
    caller: PathBuf,
}

struct Bundle {
    name: String,
    dir: PathBuf,
    metadata: BTreeMap<String, String>,
    hashes: BTreeMap<String, String>,
    has_deploy: bool,
}

struct Link {
    name: &'static str,
    before: Option<PathBuf>,
    target: Option<PathBuf>,
    description: &'static str,
}

impl Link {
    fn changes(&self) -> bool {
        self.target.is_some() && self.target != self.before
    }
}

struct Install {
    id: String,
    title: String,
    program: PathBuf,
    args: Vec<String>,
    items: Option<Vec<Value>>,
    warnings: Vec<String>,
    plugin: bool,
}

impl Install {
    fn changes(&self) -> bool {
        self.items.as_ref().is_some_and(|items| {
            items.iter().any(|item| {
                item["status"] == "overwrite" || !self.plugin && item["status"] == "create"
            })
        })
    }
}

struct Plan {
    paths: Paths,
    bundle: Bundle,
    links: Vec<Link>,
    installs: Vec<Install>,
    before_version: Option<String>,
    rollback: String,
    notes: Vec<String>,
    pony: Option<pony::Plan>,
    history: Vec<Value>,
}

pub fn run(args: &[&str]) -> u8 {
    run_with(args, &pony::System::native())
}

pub(crate) fn run_with(args: &[&str], system: &pony::System) -> u8 {
    let (mut version, mut dry, mut yes) = (None, false, false);
    for arg in args {
        match *arg {
            "--dry-run" => dry = true,
            "--yes" => yes = true,
            name if !name.starts_with('-') && version.is_none() => version = Some(name),
            _ => {
                println!("用法：ranch deploy <版本> [--dry-run] [--yes]");
                return 2;
            }
        }
    }
    let Some(version) = version else {
        println!("用法：ranch deploy <版本> [--dry-run] [--yes]");
        return 2;
    };
    match plan(version, system).and_then(|plan| preview(&plan, dry, yes, system)) {
        Ok(()) => 0,
        Err(error) => {
            println!("部署停止：{error}");
            1
        }
    }
}

fn error(path: &Path, error: io::Error) -> String {
    format!("{}: {error}", path.display())
}

fn hash(path: &Path) -> Result<String> {
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| error(path, e))?;
    if !file.metadata().map_err(|e| error(path, e))?.is_file() {
        return Err(format!("{} 不是普通程序文件", path.display()));
    }
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let len = file.read(&mut buffer).map_err(|e| error(path, e))?;
        if len == 0 {
            break;
        }
        hash.update(&buffer[..len]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn target(paths: &Paths, name: &str) -> Result<Bundle> {
    if name.is_empty() || name.starts_with('.') || name.contains('/') {
        return Err("版本必须是 versions/ 下直接存在的目录名，不含 /、不以 . 开头".into());
    }
    let dir = paths.versions.join(name);
    if !fs::symlink_metadata(&dir)
        .map_err(|e| error(&dir, e))?
        .is_dir()
    {
        return Err(format!("{} 不是版本目录", dir.display()));
    }
    let build = dir.join("BUILD.txt");
    let text = fs::read_to_string(&build).map_err(|e| error(&build, e))?;
    let mut metadata = BTreeMap::new();
    let mut expected = BTreeMap::new();
    for line in text.lines() {
        if let Some((key, value)) = line.split_once(": ") {
            metadata.insert(key.to_owned(), value.to_owned());
        } else if let Some((digest, file)) = line.split_once("  ")
            && (digest.len() != 64
                || !digest.bytes().all(|b| b.is_ascii_hexdigit())
                || expected
                    .insert(file.to_owned(), digest.to_lowercase())
                    .is_some())
        {
            return Err("BUILD.txt 校验和格式错误或重复".into());
        }
    }
    for key in ["revision", "target", "working-tree", "branch"] {
        if !metadata.contains_key(key) {
            return Err(format!("BUILD.txt 缺少 {key}"));
        }
    }
    let bin = dir.join("bin");
    let mut hashes = BTreeMap::new();
    for entry in fs::read_dir(&bin).map_err(|e| error(&bin, e))? {
        let entry = entry.map_err(|e| error(&bin, e))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !PROGRAMS.contains(&name.as_str()) {
            return Err(format!(
                "不认识的程序 {name}；请用 {} deploy {}",
                quote(&dir.join("bin/ranch")),
                shell_words::quote(&dir.file_name().unwrap().to_string_lossy())
            ));
        }
        if !entry
            .file_type()
            .map_err(|e| error(&entry.path(), e))?
            .is_file()
        {
            return Err(format!("{} 不是普通程序文件", entry.path().display()));
        }
        let digest = hash(&entry.path())?;
        if expected.get(&format!("bin/{name}")) != Some(&digest) {
            return Err(format!("{name} 和 BUILD.txt 的校验和不一致或缺失"));
        }
        hashes.insert(name, digest);
    }
    for file in expected.keys() {
        if let Some(name) = file.strip_prefix("bin/")
            && !hashes.contains_key(name)
        {
            return Err(format!("BUILD.txt 列出的 {file} 缺失，版本目录被改过"));
        }
    }
    if !hashes.contains_key("corral") {
        return Err("目标版本没有 corral".into());
    }
    let mut has_deploy = false;
    for name in hashes.keys() {
        let args = vec![
            if name == "ranch" {
                "--help"
            } else {
                "--version"
            }
            .into(),
        ];
        let output = command::run(&bin.join(name), &args, Duration::from_secs(5))?;
        if name == "ranch" {
            has_deploy = String::from_utf8_lossy(&output.stdout).contains("ranch deploy");
            if output.status.success() {
                continue;
            }
        } else if output.status.success()
            && serde_json::from_slice::<Value>(&output.stdout)
                .is_ok_and(|v| v["ok"] == true && v["contract"] == "1")
        {
            continue;
        }
        return Err(format!(
            "{name} 无法运行或不支持 contract 1：{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(Bundle {
        name: name.into(),
        dir,
        metadata,
        hashes,
        has_deploy,
    })
}

fn read_link(path: &Path) -> Result<Option<PathBuf>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_symlink() => {
            fs::read_link(path).map(Some).map_err(|e| error(path, e))
        }
        Ok(_) => Err(format!("{} 不是符号链接，不替换", path.display())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(error(path, e)),
    }
}

fn resolved(bin: &Path, link: &Path) -> PathBuf {
    let absolute = bin.join(link);
    fs::canonicalize(&absolute).unwrap_or(absolute)
}

fn version_of(paths: &Paths, link: &Path, name: &str) -> Option<String> {
    let path = resolved(&paths.bin, link);
    let relative = path.strip_prefix(&paths.versions).ok()?;
    let parts: Vec<_> = relative.components().collect();
    if parts.len() != 3 || parts[1].as_os_str() != "bin" || parts[2].as_os_str() != name {
        return None;
    }
    let version = parts[0].as_os_str().to_str()?;
    (!version.starts_with('.')).then(|| version.to_owned())
}

fn before_version(paths: &Paths, links: &[Link]) -> Option<String> {
    let candidates: BTreeSet<_> = links
        .iter()
        .filter_map(|link| version_of(paths, link.before.as_ref()?, link.name))
        .collect();
    candidates.into_iter().find(|version| {
        links.iter().all(|link| {
            let program = paths.versions.join(version).join("bin").join(link.name);
            match &link.before {
                Some(before) => hash(&resolved(&paths.bin, before))
                    .ok()
                    .zip(hash(&program).ok())
                    .is_some_and(|(before, candidate)| before == candidate),
                None => !program.exists(),
            }
        })
    })
}

fn quote(path: &Path) -> String {
    shell_words::quote(&path.to_string_lossy()).into_owned()
}

fn deploy_command(paths: &Paths, version: &str) -> String {
    format!(
        "{} deploy {}",
        quote(&paths.caller),
        shell_words::quote(version)
    )
}

fn rollback(paths: &Paths, links: &[Link], previous: Option<&str>) -> String {
    if let Some(previous) = previous {
        return deploy_command(paths, previous);
    }
    let commands: Vec<_> = links
        .iter()
        .filter(|link| link.changes())
        .map(|link| {
            let path = quote(&paths.bin.join(link.name));
            match &link.before {
                Some(before) => format!("ln -sfn -- {} {path}", quote(before)),
                None => format!("rm -- {path}"),
            }
        })
        .collect();
    format!(
        "无法归到一个旧版本；逐个恢复链接（技能和插件需另按旧程序恢复）：\n  {}",
        commands.join("\n  ")
    )
}

/// `usage`/`unsupported` means an older executable lacks this optional command.
/// Other failures and malformed output must stop deployment, never look like `same`.
fn probe(program: &Path, args: &[String]) -> Result<Option<Value>> {
    let timeout = if args == ["integration", "status"] {
        15
    } else {
        5
    };
    let output = command::run(program, args, Duration::from_secs(timeout))?;
    let value: Value = serde_json::from_slice(&output.stdout).map_err(|e| {
        format!(
            "{} {} 输出不是 JSON：{e}",
            program.display(),
            args.join(" ")
        )
    })?;
    if !output.status.success() || value["ok"] != true {
        if matches!(value["error"].as_str(), Some("usage" | "unsupported")) {
            return Ok(None);
        }
        return Err(format!(
            "{} {} 检查失败：{value}",
            program.display(),
            args.join(" ")
        ));
    }
    Ok(Some(value))
}

fn items(value: &Value) -> Result<Vec<Value>> {
    let items = value["items"]
        .as_array()
        .ok_or("技能检查缺少 items")?
        .clone();
    for item in &items {
        status(item)?;
    }
    Ok(items)
}

fn status(item: &Value) -> Result<&str> {
    match item["status"].as_str() {
        Some(status @ ("same" | "create" | "overwrite" | "foreign")) => Ok(status),
        _ => Err(format!("不认识的安装状态：{item}")),
    }
}

fn warnings(value: &Value) -> Vec<String> {
    value["warnings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}

fn installations(bundle: &Bundle) -> Result<Vec<Install>> {
    let bin = bundle.dir.join("bin");
    let mut installs = Vec::new();
    for (program, args) in [
        ("corral", vec!["install-skills"]),
        ("ranch", vec!["dispatch", "install-skills"]),
    ] {
        if !bundle.hashes.contains_key(program) {
            continue;
        }
        let args: Vec<String> = args.into_iter().map(str::to_owned).collect();
        let mut check = args.clone();
        check.push("--dry-run".into());
        let value = probe(&bin.join(program), &check)?;
        installs.push(Install {
            id: format!("{program}_skills"),
            title: format!("{program} 技能"),
            program: bin.join(program),
            args,
            items: value.as_ref().map(items).transpose()?,
            warnings: value.as_ref().map(warnings).unwrap_or_default(),
            plugin: false,
        });
    }
    let corral = bin.join("corral");
    if let Some(value) = probe(&corral, &["integration".into(), "status".into()])? {
        let agents = value["agents"].as_array().ok_or("插件检查缺少 agents")?;
        for agent in agents {
            if agent.get("install").is_none() {
                continue;
            }
            let kind = agent["kind"].as_str().ok_or("插件检查缺少 kind")?;
            status(&agent["install"])?;
            installs.push(Install {
                id: format!("plugin_{kind}"),
                title: format!("全局插件 {kind}"),
                program: corral.clone(),
                args: vec![
                    "integration".into(),
                    "install".into(),
                    "--agent".into(),
                    kind.into(),
                ],
                items: Some(vec![agent["install"].clone()]),
                warnings: warnings(&value),
                plugin: true,
            });
        }
    } else {
        installs.push(Install {
            id: "plugins".into(),
            title: "全局插件".into(),
            program: corral,
            args: vec![],
            items: None,
            warnings: vec![],
            plugin: true,
        });
    }
    Ok(installs)
}

fn plan(name: &str, system: &pony::System) -> Result<Plan> {
    if unsafe { libc::geteuid() } == 0 || unsafe { libc::getuid() } == 0 {
        return Err("不能用 root / sudo 部署".into());
    }
    if env::var_os("FENCE_RUN").is_some() || env::var_os("CODEX_SANDBOX").is_some() {
        return Err("不能在 fence 或 Codex sandbox 里部署".into());
    }
    let home = env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .ok_or("HOME 未设置")?;
    let home = fs::canonicalize(home).map_err(|e| format!("HOME: {e}"))?;
    let state = home.join(".local/share/ranch");
    let paths = Paths {
        bin: home.join(".local/bin"),
        versions: state.join("versions"),
        state,
        // Resolve before replacing our own link: macOS current_exe may retain it.
        caller: fs::canonicalize(env::current_exe().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?,
    };
    let bundle = target(&paths, name)?;
    let mut links = Vec::new();
    let mut notes = Vec::new();
    for name in PROGRAMS {
        let before = read_link(&paths.bin.join(name))?;
        let target = bundle
            .hashes
            .get(name)
            .map(|_| bundle.dir.join("bin").join(name));
        let description = match (&before, &target) {
            (_, None) => "目标版本没有，不动这个链接",
            (None, Some(_)) => "新建",
            (Some(a), Some(b)) if a == b => "不变",
            (Some(a), Some(_))
                if hash(&resolved(&paths.bin, a)).ok().as_ref() == bundle.hashes.get(name) =>
            {
                "程序相同"
            }
            _ => "程序变了",
        };
        links.push(Link {
            name,
            before,
            target,
            description,
        });
    }
    let history = read_history(&paths.state.join("deploys.jsonl"))?;
    let mut previous = before_version(&paths, &links);
    let mut rollback = rollback(&paths, &links, previous.as_deref());
    let unfinished =
        history.iter().rev().find(|r| r.is_object()).filter(|r| {
            r["to"] == name && matches!(r["result"].as_str(), Some("started" | "failed"))
        });
    let prior = unfinished.or_else(|| {
        if previous.as_deref() == Some(name) {
            history.iter().rev().find(|r| r["to"] == name)
        } else {
            None
        }
    });
    if let Some(prior) = prior
        && let Some(saved) = prior["rollback"].as_str().map(str::to_owned).or_else(|| {
            prior["before_version"]
                .as_str()
                .map(|version| deploy_command(&paths, version))
        })
    {
        previous = prior["before_version"].as_str().map(str::to_owned);
        rollback = saved;
        notes.push(
            if unfinished.is_some() {
                "接着上次未完成的部署，退回沿用那次"
            } else {
                "当前已是目标版本，退回沿用最近一次部署记录"
            }
            .into(),
        );
    }
    let pony = pony::plan(&bundle, &home, system)?;
    notes.push(
        pony.as_ref()
            .map_or("目标版本没有 pony，不动服务".into(), |p| {
                p.note.clone()
            }),
    );
    let helper = env::var_os("CORRAL_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".corral"))
        .join(".runtime/helper");
    let detail = match fs::read_link(&helper) {
        Ok(link) => {
            let program = helper.parent().unwrap().join(&link);
            let comparison = if hash(&program).ok().as_ref() == bundle.hashes.get("corral") {
                "相同"
            } else {
                "不同或无法读取"
            };
            format!("指 {}，和这一版的 corral {comparison}", link.display())
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => "未安装".into(),
        Err(e) => format!("无法读取：{e}"),
    };
    notes.push(format!(
        "共用 helper {detail}；deploy 不切，要换见 HANDOFF「部署时要记得」"
    ));
    notes.push("在跑的 agent 不动，不 corral upgrade；新开的 agent 用新版".into());
    let installs = installations(&bundle)?;
    Ok(Plan {
        paths,
        bundle,
        links,
        installs,
        before_version: previous,
        rollback,
        notes,
        pony,
        history,
    })
}

fn read_history(path: &Path) -> Result<Vec<Value>> {
    let text = match fs::read(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(error(path, e)),
    };
    Ok(text
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
        .map(|line| serde_json::from_slice(line).unwrap_or(Value::Null))
        .collect())
}

// Records use UTC; only the human-facing plan uses the machine's local zone.
fn local_time(at: &str) -> String {
    let Ok(input) = CString::new(at) else {
        return at.into();
    };
    let mut utc: libc::tm = unsafe { std::mem::zeroed() };
    let end = unsafe { libc::strptime(input.as_ptr(), c"%Y-%m-%dT%H:%M:%SZ".as_ptr(), &mut utc) };
    if end.is_null() || unsafe { *end } != 0 {
        return at.into();
    }
    let seconds = unsafe { libc::timegm(&mut utc) };
    let mut local: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&seconds, &mut local) }.is_null() {
        return at.into();
    }
    let mut text = [0u8; 64];
    let len = unsafe {
        libc::strftime(
            text.as_mut_ptr().cast(),
            text.len(),
            c"%Y-%m-%d %H:%M:%S %z".as_ptr(),
            &local,
        )
    };
    if len == 0 {
        return at.into();
    }
    String::from_utf8_lossy(&text[..len]).into_owned()
}

fn last_record(history: &[Value]) -> String {
    let Some(value) = history.last() else {
        return "没有记录".into();
    };
    if !value.is_object() {
        return "末行不完整（可能被中断）".into();
    }
    let result = match value["result"].as_str() {
        Some("started") => "没有结束记录（可能被中断）",
        Some("done") => "做完",
        Some("failed") => "失败",
        _ => "不认识的记录",
    };
    format!(
        "{} {} → {}，{result}",
        local_time(value["at"].as_str().unwrap_or("?")),
        value["before_version"].as_str().unwrap_or("混合或未安装"),
        value["to"].as_str().unwrap_or("?")
    )
}

fn preview(plan: &Plan, dry: bool, yes: bool, system: &pony::System) -> Result<()> {
    let bundle = &plan.bundle;
    println!(
        "部署 {}（提交 {}，分支 {}，{}，工作区 {}）",
        bundle.name,
        bundle.metadata["revision"],
        bundle.metadata["branch"],
        bundle.metadata["target"],
        bundle.metadata["working-tree"]
    );
    println!(
        "{} 个程序和 BUILD.txt 的校验和一致，都能运行。\n\n要做的：",
        bundle.hashes.len()
    );
    let mut count = 0;
    if plan.links.iter().any(Link::changes) {
        count += 1;
        println!("  {count}. 切链接（{}）", plan.paths.bin.display());
        for link in plan.links.iter().filter(|l| l.changes()) {
            let before = link_record(&plan.paths, link.name, &link.before);
            println!(
                "       {}  {} → {}  {}",
                link.name,
                before.as_str().unwrap_or("未安装"),
                bundle.name,
                link.description
            );
        }
    }
    for install in plan.installs.iter().filter(|i| i.changes()) {
        count += 1;
        println!("  {count}. 重装{}", install.title);
    }
    if let Some(pony) = &plan.pony
        && pony.action.is_some()
    {
        count += 1;
        println!("  {count}. {}", pony.description());
    }
    println!("\n不做的：");
    for link in plan.links.iter().filter(|l| !l.changes()) {
        println!("  {} 链接：{}", link.name, link.description);
    }
    for install in &plan.installs {
        match &install.items {
            None => println!("  {}：目标版本不支持检查，跳过", install.title),
            Some(items) => {
                if !install.changes() {
                    let states: BTreeSet<_> =
                        items.iter().filter_map(|i| i["status"].as_str()).collect();
                    println!(
                        "  {}：{}，{}",
                        install.title,
                        states.into_iter().collect::<Vec<_>>().join("、"),
                        if install.plugin && items.iter().any(|i| i["status"] == "create") {
                            "没装，不装"
                        } else {
                            "不重装"
                        }
                    );
                }
                for item in items.iter().filter(|i| i["status"] == "foreign") {
                    println!(
                        "  警告：{} {} 是 foreign，不动",
                        install.title, item["path"]
                    );
                }
            }
        }
        for warning in &install.warnings {
            println!("  警告：{warning}");
        }
    }
    for note in &plan.notes {
        println!("  {note}");
    }
    println!("\n上次部署：{}", last_record(&plan.history));
    println!("做完后退回：{}", plan.rollback);
    if count == 0 {
        println!("已经是 {}，没有要做的", bundle.name);
        return Ok(());
    }
    if dry {
        return Ok(());
    }
    if !yes {
        if !io::stdin().is_terminal() {
            return Err(
                "confirmation_required：没有终端，没法确认，什么都没改；看过计划后加 --yes 再跑"
                    .into(),
            );
        }
        print!("确认做以上 {count} 步？[y/N] ");
        io::stdout().flush().map_err(|e| e.to_string())?;
        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .map_err(|e| e.to_string())?;
        if !answer.trim().eq_ignore_ascii_case("y") {
            return Err("已取消，什么都没改".into());
        }
    }
    apply(plan, system)
}

fn unchanged(plan: &Plan) -> Result<()> {
    for link in &plan.links {
        if read_link(&plan.paths.bin.join(link.name))? != link.before {
            return Err(format!(
                "{} 链接在计划后变了，什么都没部署；请重跑",
                link.name
            ));
        }
    }
    for (name, expected) in &plan.bundle.hashes {
        if hash(&plan.bundle.dir.join("bin").join(name))? != *expected {
            return Err(format!("{name} 在计划后变了，什么都没部署；请重跑"));
        }
    }
    Ok(())
}

fn switch_links(plan: &Plan) -> Result<()> {
    fs::create_dir_all(&plan.paths.bin).map_err(|e| error(&plan.paths.bin, e))?;
    for link in plan.links.iter().filter(|l| l.changes()) {
        let path = plan.paths.bin.join(link.name);
        if read_link(&path)? != link.before {
            return Err(format!("{} 链接在计划后变了；请重跑", link.name));
        }
        let temporary = tempfile::Builder::new()
            .prefix(".ranch-deploy-")
            .tempdir_in(&plan.paths.bin)
            .map_err(|e| error(&plan.paths.bin, e))?;
        let staged = temporary.path().join(link.name);
        symlink(link.target.as_ref().unwrap(), &staged).map_err(|e| error(&staged, e))?;
        // An absent destination must stay absent. hard_link copies the symlink inode,
        // without following it, and fails if someone created the destination meanwhile.
        if link.before.is_none() {
            fs::hard_link(&staged, &path).map_err(|e| error(&path, e))?;
        } else {
            if read_link(&path)? != link.before {
                return Err(format!("{} 链接在计划后变了；请重跑", link.name));
            }
            fs::rename(&staged, &path).map_err(|e| error(&path, e))?;
        }
    }
    fs::File::open(&plan.paths.bin)
        .and_then(|f| f.sync_all())
        .map_err(|e| error(&plan.paths.bin, e))
}

fn recheck(install: &Install) -> Result<Vec<Value>> {
    let mut args = install.args.clone();
    args.push("--dry-run".into());
    let value =
        probe(&install.program, &args)?.ok_or_else(|| format!("{} 不再支持检查", install.title))?;
    for warning in warnings(&value) {
        println!("  警告：{warning}");
    }
    let items = if install.plugin {
        status(&value)?;
        vec![value]
    } else {
        items(&value)?
    };
    for item in &items {
        if item["status"] == "foreign" {
            println!(
                "  警告：{} {} 是 foreign，不动",
                install.title, item["path"]
            );
        }
    }
    Ok(items)
}

fn install(install: &Install) -> Result<()> {
    let items = recheck(install)?;
    let changes = items
        .iter()
        .any(|i| i["status"] == "overwrite" || !install.plugin && i["status"] == "create");
    if !changes {
        return Ok(());
    }
    let mut args = install.args.clone();
    args.push("--yes".into());
    let output = command::run(&install.program, &args, Duration::from_secs(30))?;
    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("{} 安装输出不是 JSON：{e}", install.title))?;
    if !output.status.success() || value["ok"] != true {
        return Err(format!(
            "{}：{value} {}",
            install.title,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    for warning in warnings(&value) {
        println!("  警告：{warning}");
    }
    Ok(())
}

fn verify(plan: &Plan, system: &pony::System) -> Result<()> {
    for link in &plan.links {
        let expected = link.target.as_ref().or(link.before.as_ref());
        if read_link(&plan.paths.bin.join(link.name))?.as_ref() != expected {
            return Err(format!("{} 链接核对失败", link.name));
        }
    }
    for install in &plan.installs {
        let Some(before) = &install.items else {
            continue;
        };
        if install.plugin
            && before
                .iter()
                .all(|i| i["status"] == "create" || i["status"] == "foreign")
        {
            continue;
        }
        if recheck(install)?
            .iter()
            .any(|i| i["status"] != "same" && i["status"] != "foreign")
        {
            return Err(format!(
                "{} 核对失败：仍有 create / overwrite",
                install.title
            ));
        }
    }
    if let Some(pony) = &plan.pony {
        pony.verify(system)?;
    }
    println!("核对：链接均符合计划；支持检查的技能、已装插件都是 same（foreign 保留）。");
    Ok(())
}

fn link_record(paths: &Paths, name: &str, link: &Option<PathBuf>) -> Value {
    match link {
        Some(link) => json!(
            version_of(paths, link, name)
                .unwrap_or_else(|| resolved(&paths.bin, link).display().to_string())
        ),
        None => Value::Null,
    }
}

fn now() -> Result<String> {
    let seconds = unsafe { libc::time(std::ptr::null_mut()) };
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::gmtime_r(&seconds, &mut tm) }.is_null() {
        return Err("无法读取部署时间".into());
    }
    Ok(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    ))
}

fn record(file: &mut fs::File, mut value: Value) -> Result<()> {
    value["at"] = json!(now()?);
    let mut bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("部署记录写入失败：{e}"))
}

fn apply(plan: &Plan, system: &pony::System) -> Result<()> {
    let lock_path = plan.paths.state.join("deploy.lock");
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&lock_path)
        .map_err(|e| error(&lock_path, e))?;
    if !lock.metadata().map_err(|e| error(&lock_path, e))?.is_file() {
        return Err("deploy.lock 不是普通文件".into());
    }
    lock.try_lock()
        .map_err(|e| format!("拿不到部署锁（可能另一个 deploy 在执行）：{e}"))?;
    unchanged(plan)?;
    let log_path = plan.paths.state.join("deploys.jsonl");
    let mut log = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&log_path)
        .map_err(|e| error(&log_path, e))?;
    if !log.metadata().map_err(|e| error(&log_path, e))?.is_file() {
        return Err("deploys.jsonl 不是普通文件".into());
    }
    let before: BTreeMap<_, _> = plan
        .links
        .iter()
        .map(|l| (l.name, link_record(&plan.paths, l.name, &l.before)))
        .collect();
    record(
        &mut log,
        json!({"by":plan.paths.caller,"to":plan.bundle.name,"before":before,"before_version":plan.before_version,"rollback":plan.rollback,"pony":plan.pony.as_ref().map(pony::Plan::record),"result":"started"}),
    )?;
    let mut steps = Vec::new();
    if plan.links.iter().any(Link::changes) {
        steps.push(json!({"step":"links","title":"切链接","result":"not_done"}));
    }
    for install in plan.installs.iter().filter(|i| i.changes()) {
        steps.push(json!({"step":install.id,"title":install.title,"result":"not_done"}));
    }
    if let Some(pony) = &plan.pony
        && let Some(action) = pony.action
    {
        steps.push(json!({"step":action,"title":pony.title(),"result":"not_done"}));
    }
    steps.push(json!({"step":"verify","title":"核对","result":"not_done"}));
    let mut failure = None;
    for step in &mut steps {
        let id = step["step"].as_str().unwrap();
        let result = match id {
            "links" => switch_links(plan),
            "verify" => verify(plan, system),
            "pony_restart" | "pony_service" => {
                plan.pony.as_ref().unwrap().execute(system).map(|value| {
                    step["pid"] = value["pid"].clone();
                    step["chats"] = value["chats"].clone();
                })
            }
            _ => install(plan.installs.iter().find(|i| i.id == id).unwrap()),
        };
        match result {
            Ok(()) => {
                step["result"] = json!("done");
                println!("{} …… 完成", step["title"].as_str().unwrap());
            }
            Err(e) => {
                step["result"] = json!("failed");
                step["message"] = json!(e);
                failure = Some(e);
                break;
            }
        }
    }
    let after: BTreeMap<_, _> = PROGRAMS
        .into_iter()
        .map(|name| {
            let value = match read_link(&plan.paths.bin.join(name)) {
                Ok(link) => link_record(&plan.paths, name, &link),
                Err(e) => json!({"error":e}),
            };
            (name, value)
        })
        .collect();
    let mut pony_after = Value::Null;
    if failure.is_some() {
        if let Some(pony) = &plan.pony {
            pony_after = pony.failure_status(system);
        }
        println!("\n中途失败，不自动撤回：");
        for step in &steps {
            let result = match step["result"].as_str().unwrap() {
                "done" => "完成",
                "failed" => "失败",
                _ => "没做",
            };
            println!(
                "  {result} {} {}",
                step["title"].as_str().unwrap(),
                step["message"].as_str().unwrap_or("")
            );
        }
        for (name, link) in &after {
            let text = link
                .as_str()
                .or_else(|| link["error"].as_str())
                .unwrap_or("未安装");
            println!("  当前链接 {name}：{text}");
        }
        println!(
            "接着做：找到原因后重跑 {}，做过的会跳过",
            deploy_command(&plan.paths, &plan.bundle.name)
        );
    }
    // Always print recovery instructions, including when the final record cannot be saved.
    println!("退回：{}", plan.rollback);
    if plan.bundle.hashes.contains_key("ranch") && !plan.bundle.has_deploy {
        println!(
            "PATH 上的 ranch 现在没有 deploy，再部署用 {}",
            quote(&plan.paths.caller)
        );
    }
    record(
        &mut log,
        json!({"to":plan.bundle.name,"before_version":plan.before_version,"after":after,"steps":steps,"pony":plan.pony.as_ref().map(pony::Plan::record),"pony_after":pony_after,"result":if failure.is_some() {"failed"} else {"done"},"rollback":plan.rollback}),
    )?;
    println!("已记到 {}", log_path.display());
    failure.map_or(Ok(()), Err)
}
