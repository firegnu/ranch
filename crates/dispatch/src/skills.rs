//! `ranch dispatch install-skills`: writes the corral-dispatch skill for Claude Code and Codex.
//! Follows corral's `install-skills` (consent, dry run, symlinks left alone); new in ranch,
//! replacing Saddle's plugin resource installer.
use serde_json::{Value, json};
use std::{
    fs,
    io::{IsTerminal, Write},
    path::{Path, PathBuf},
};

/// A failed install: the exit code and the JSON to print.
pub type Failure = (u8, Value);

fn fail(code: &str, message: &str) -> Failure {
    (1, json!({"ok":false,"error":code,"message":message}))
}

fn linked(path: &Path, base: &Path) -> bool {
    let mut p = Some(path);
    while let Some(q) = p {
        if fs::symlink_metadata(q).is_ok_and(|m| m.file_type().is_symlink()) {
            return true;
        }
        if q == base {
            break;
        }
        p = q.parent();
    }
    false
}

/// Installs the skill under `~/.claude` (or `CLAUDE_CONFIG_DIR`) and `~/.agents`.
pub fn install(target: &str, dry: bool, yes: bool) -> Result<Value, Failure> {
    if !["all", "claude", "codex"].contains(&target) {
        return Err(fail("usage", "invalid skill target"));
    }
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let mut items = Vec::new();
    let mut warnings = Vec::new();
    for agent in ["claude", "codex"] {
        if target != "all" && target != agent {
            continue;
        }
        let base = if agent == "claude" {
            std::env::var_os("CLAUDE_CONFIG_DIR")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".claude"))
        } else {
            home.join(".agents")
        };
        let dir = base.join("skills/corral-dispatch");
        for (name, body) in crate::SKILL {
            let path = dir.join(name);
            let status = if linked(&path, &base) {
                warnings.push(format!(
                    "{} is not an owned regular skill file; left in place",
                    path.display()
                ));
                "foreign"
            } else {
                match fs::read(&path) {
                    Ok(current) if current == *body => "same",
                    Ok(_) => "overwrite",
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => "create",
                    Err(e) => return Err(fail("io", &format!("{}: {e}", path.display()))),
                }
            };
            items.push(json!({"agent":agent,"path":path,"status":status}));
        }
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if !crate::SKILL.iter().any(|(n, _)| name == *n) {
                    warnings.push(format!(
                        "{} is not part of this skill; left in place",
                        entry.path().display()
                    ));
                }
            }
        }
    }
    let mut result = json!({"ok":true,"action":"install","dry_run":dry,"written":false,"items":items,"warnings":warnings});
    let todo: Vec<_> = items
        .iter()
        .filter(|i| matches!(i["status"].as_str(), Some("create" | "overwrite")))
        .collect();
    if dry || todo.is_empty() {
        return Ok(result);
    }
    if !yes {
        if !std::io::stdin().is_terminal() {
            return Err((
                1,
                json!({"ok":false,"error":"confirmation_required","message":"install-skills writes skill files; confirm interactively or pass --yes after user approval","items":items,"warnings":warnings}),
            ));
        }
        eprintln!("ranch dispatch install-skills 将写入：");
        for item in &todo {
            eprintln!(
                "  [{}] {}",
                item["status"].as_str().unwrap(),
                item["path"].as_str().unwrap()
            );
        }
        eprint!("确认写入以上文件？[y/N] ");
        let _ = std::io::stderr().flush();
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_err()
            || !["y", "yes"].contains(&line.trim().to_lowercase().as_str())
        {
            return Err(fail("declined", "nothing written"));
        }
    }
    for item in todo {
        let path = PathBuf::from(item["path"].as_str().unwrap());
        let name = path.file_name().unwrap();
        let body = crate::SKILL.iter().find(|(n, _)| name == *n).unwrap().1;
        write(&path, body).map_err(|e| fail("io", &format!("{}: {e}", path.display())))?;
    }
    result["written"] = json!(true);
    Ok(result)
}

/// Writes through a temporary file in the same directory, then renames over the target.
fn write(path: &Path, body: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().expect("skill file has a directory");
    fs::create_dir_all(parent)?;
    let temp = parent.join(format!(".ranch-skill-{}", std::process::id()));
    let mut f = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)?;
    f.write_all(body)?;
    drop(f);
    fs::rename(&temp, path).inspect_err(|_| {
        let _ = fs::remove_file(&temp);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    // HOME is process-wide; tests that change it run one at a time.
    static ENV: Mutex<()> = Mutex::new(());

    fn with_home<T>(f: impl FnOnce(&Path) -> T) -> T {
        let _guard = ENV.lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        // SAFETY: serialized by ENV; no other thread reads these variables meanwhile.
        unsafe {
            std::env::set_var("HOME", home.path());
            std::env::remove_var("CLAUDE_CONFIG_DIR");
        }
        f(home.path())
    }

    #[test]
    fn creates_then_reports_same_and_leaves_extra_files() {
        with_home(|home| {
            let first = install("all", false, true).unwrap();
            assert_eq!(first["written"], true);
            let statuses: Vec<_> = first["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i["status"].as_str().unwrap().to_owned())
                .collect();
            assert_eq!(statuses, vec!["create"; 6]);
            let skill = home.join(".claude/skills/corral-dispatch/SKILL.md");
            assert_eq!(fs::read(&skill).unwrap(), crate::SKILL[0].1);
            fs::write(
                home.join(".agents/skills/corral-dispatch/遥测操作.md"),
                "old",
            )
            .unwrap();
            let again = install("all", false, true).unwrap();
            assert_eq!(again["written"], false);
            assert!(
                again["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|i| i["status"] == "same")
            );
            assert!(
                again["warnings"][0]
                    .as_str()
                    .unwrap()
                    .contains("遥测操作.md")
            );
            assert!(
                home.join(".agents/skills/corral-dispatch/遥测操作.md")
                    .exists()
            );
        });
    }

    #[test]
    fn dry_run_and_missing_consent_write_nothing() {
        with_home(|home| {
            let dry = install("codex", true, false).unwrap();
            assert_eq!(dry["written"], false);
            assert_eq!(dry["items"].as_array().unwrap().len(), 3);
            assert!(!home.join(".agents").exists());
            // stdin is not a terminal under the test harness.
            let (code, value) = install("claude", false, false).unwrap_err();
            assert_eq!(code, 1);
            assert_eq!(value["error"], "confirmation_required");
            assert!(!home.join(".claude").exists());
        });
    }

    #[test]
    fn symlinked_skill_directories_are_left_alone() {
        with_home(|home| {
            let elsewhere = home.join("elsewhere");
            fs::create_dir_all(&elsewhere).unwrap();
            fs::create_dir_all(home.join(".claude/skills")).unwrap();
            std::os::unix::fs::symlink(&elsewhere, home.join(".claude/skills/corral-dispatch"))
                .unwrap();
            let result = install("claude", false, true).unwrap();
            assert_eq!(result["written"], false);
            assert!(
                result["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|i| i["status"] == "foreign")
            );
            assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0);
        });
    }

    #[test]
    fn overwrites_changed_files_and_rejects_bad_targets() {
        with_home(|home| {
            let readme = home.join(".agents/skills/corral-dispatch/README.md");
            fs::create_dir_all(readme.parent().unwrap()).unwrap();
            fs::write(&readme, "older").unwrap();
            let result = install("codex", false, true).unwrap();
            assert_eq!(result["items"][2]["status"], "overwrite");
            assert_eq!(fs::read(&readme).unwrap(), crate::SKILL[2].1);
            assert_eq!(install("vim", true, true).unwrap_err().1["error"], "usage");
        });
    }
}
