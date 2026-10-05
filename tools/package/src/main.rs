//! Build an immutable ranch version directory. Never installs, switches links or installs
//! skills: those are separate deployment steps. Follows Saddle's `scripts/package.sh` at
//! `a31dea2`, limited to corral.
//!
//! Usage: `cargo run -p ranch-package [-- NEW_OUTPUT_DIRECTORY]`
//! Default output: `~/.local/share/ranch/versions/<short commit>/`.
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, exit},
};

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
    for dir in ["bin", "share/corral"] {
        fs::create_dir_all(staging.0.join(dir)).map_err(|e| e.to_string())?;
    }
    copy(
        &target_dir.join(&host).join("release/corral"),
        &staging.0.join("bin/corral"),
    )?;
    for file in SHARED {
        let from = root.join(file);
        let name = from.file_name().ok_or("bad shared path")?;
        copy(&from, &staging.0.join("share/corral").join(name))?;
    }

    let revision = output_of(&root, "git", &["rev-parse", "HEAD"])?;
    let dirty = !output_of(&root, "git", &["status", "--porcelain"])?.is_empty();
    let branch = output_of(
        &root,
        "git",
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
    )
    .unwrap_or_default();
    let digest = output_of(&staging.0, "shasum", &["-a", "256", "bin/corral"])?;
    let build = format!(
        "revision: {revision}\ntarget: {host}\nworking-tree: {}\nsource: {}\nbranch: {branch}\n{digest}\n",
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
