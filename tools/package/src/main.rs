//! Build an immutable ranch version directory. Never installs, switches links or installs
//! skills: those are separate deployment steps. Follows Saddle's `scripts/package.sh` at
//! `a31dea2`; ranch now also packages the independent fence runtime.
//!
//! Usage: `cargo run -p ranch-package [-- NEW_OUTPUT_DIRECTORY]`
//! Default output: `~/.local/share/ranch/versions/<short commit>/`.
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, exit},
};

/// Programs placed in `bin/`.
const PROGRAMS: &[&str] = &["corral", "ranch", "fence"];

/// Files shipped beside the program, from the repository root.
const SHARED: &[&str] = &[
    "crates/corral/resources/SKILL.md",
    "crates/corral/resources/AGENT_USAGE.md",
    "crates/corral/resources/UPGRADING.md",
    "crates/corral/resources/hook_pi.ts",
    "crates/corral/resources/hook_omp.ts",
    "docs/Corral核心Rust集成设计.md",
    "docs/Corral通用升级设计.md",
];

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        exit(1);
    }
}

fn run() -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = fs::canonicalize(&root).map_err(|e| format!("repository root: {e}"))?;
    let args: Vec<String> = env::args().skip(1).collect();
    let output = match args.as_slice() {
        [] => {
            let home = env::var_os("HOME").ok_or("HOME is not set")?;
            let rev = output_of(&root, "git", &["rev-parse", "--short=7", "HEAD"])?;
            PathBuf::from(home)
                .join(".local/share/ranch/versions")
                .join(rev)
        }
        [dir] => std::path::absolute(dir).map_err(|e| format!("{dir}: {e}"))?,
        _ => return Err("Usage: ranch-package [NEW_OUTPUT_DIRECTORY]".into()),
    };
    refuse_existing(&output)?;

    let runtime_error = "打包 fence 需要 node ≥ 22.12 和 npm";
    let node = output_of(&root, "node", &["-p", "process.execPath"])
        .map_err(|e| format!("{runtime_error}: {e}"))?;
    let node_version =
        output_of(&root, &node, &["--version"]).map_err(|e| format!("{runtime_error}: {e}"))?;
    let mut parts = node_version
        .trim_start_matches('v')
        .split('.')
        .filter_map(|s| s.parse::<u32>().ok());
    if !Path::new(&node).is_absolute()
        || (parts.next().unwrap_or(0), parts.next().unwrap_or(0)) < (22, 12)
    {
        return Err(runtime_error.into());
    }
    output_of(&root, "npm", &["--version"]).map_err(|e| format!("{runtime_error}: {e}"))?;

    let host = output_of(&root, "rustc", &["-vV"])?
        .lines()
        .find_map(|l| l.strip_prefix("host: ").map(str::to_owned))
        .ok_or("rustc -vV printed no host")?;
    let target_dir = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    let target_dir = std::path::absolute(target_dir).map_err(|e| e.to_string())?;
    let status = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .current_dir(&root)
        .args([
            "build",
            "-p",
            "corral-core",
            "--bin",
            "corral",
            "-p",
            "ranch",
            "--bin",
            "ranch",
            "-p",
            "fence",
            "--bin",
            "fence",
            "--release",
            "--locked",
        ])
        .args(["--target", &host, "--target-dir"])
        .arg(&target_dir)
        .status()
        .map_err(|e| format!("cargo: {e}"))?;
    if !status.success() {
        return Err("cargo build failed; nothing written".into());
    }

    let parent = output.parent().ok_or("output has no parent directory")?;
    fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    let staging = Staging(parent.join(format!(".ranch-package.{}", std::process::id())));
    fs::create_dir(&staging.0).map_err(|e| format!("{}: {e}", staging.0.display()))?;
    for dir in ["bin", "share/corral", "share/fence", "lib/srt"] {
        fs::create_dir_all(staging.0.join(dir)).map_err(|e| e.to_string())?;
    }
    for program in PROGRAMS {
        copy(
            &target_dir.join(&host).join("release").join(program),
            &staging.0.join("bin").join(program),
        )?;
    }
    for file in SHARED {
        let from = root.join(file);
        let name = from.file_name().ok_or("bad shared path")?;
        copy(&from, &staging.0.join("share/corral").join(name))?;
    }
    copy(
        &root.join("crates/fence/resources/runner.mjs"),
        &staging.0.join("share/fence/runner.mjs"),
    )?;
    for file in ["package.json", "package-lock.json"] {
        copy(
            &root.join("crates/fence/srt").join(file),
            &staging.0.join("lib/srt").join(file),
        )?;
    }
    let status = Command::new("npm")
        .args([
            "ci",
            "--omit=dev",
            "--ignore-scripts",
            "--no-audit",
            "--no-fund",
            "--prefix",
        ])
        .arg(staging.0.join("lib/srt"))
        .status()
        .map_err(|e| format!("npm: {e}"))?;
    if !status.success() {
        return Err("npm ci failed; nothing installed".into());
    }
    let runtime = toml::Table::from_iter([
        ("node".into(), toml::Value::String(node.clone())),
        (
            "node_version".into(),
            toml::Value::String(node_version.clone()),
        ),
        ("srt_version".into(), toml::Value::String("0.0.80".into())),
    ]);
    fs::write(
        staging.0.join("share/fence/runtime.toml"),
        toml::to_string(&runtime).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    let revision = output_of(&root, "git", &["rev-parse", "HEAD"])?;
    let dirty = !output_of(&root, "git", &["status", "--porcelain"])?.is_empty();
    let branch = output_of(
        &root,
        "git",
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
    )
    .unwrap_or_default();
    let digest = output_of(
        &staging.0,
        "shasum",
        &["-a", "256", "bin/corral", "bin/ranch", "bin/fence"],
    )?;
    let build = format!(
        "revision: {revision}\ntarget: {host}\nworking-tree: {}\nsource: {}\nbranch: {branch}\nsrt: 0.0.80\nnode: {node_version} ({node})\n{digest}\n",
        if dirty { "modified" } else { "clean" },
        root.display(),
    );
    fs::write(staging.0.join("BUILD.txt"), build).map_err(|e| e.to_string())?;

    refuse_existing(&output).map_err(|_| "Output appeared during build; nothing installed")?;
    fs::rename(&staging.0, &output).map_err(|e| format!("{}: {e}", output.display()))?;
    println!(
        "Product directory: {}\nNo global installation or registration performed.",
        output.display()
    );
    Ok(())
}

/// Removes the half-built directory unless it was renamed into place.
struct Staging(PathBuf);

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn refuse_existing(path: &Path) -> Result<(), String> {
    if fs::symlink_metadata(path).is_ok() {
        return Err(format!(
            "Refusing to overwrite an existing product directory: {}",
            path.display()
        ));
    }
    Ok(())
}

fn copy(from: &Path, to: &Path) -> Result<(), String> {
    fs::copy(from, to)
        .map(drop)
        .map_err(|e| format!("{} -> {}: {e}", from.display(), to.display()))
}

/// Runs a command in `dir` and returns its trimmed standard output.
fn output_of(dir: &Path, program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program)
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| format!("{program}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{program} {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}
