use crate::{Error, Result};
use serde_json::{Value, json};
use std::{
    fs,
    io::{IsTerminal, Write},
    path::{Path, PathBuf},
};
const BODY: &str = include_str!("../resources/SKILL.md");
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
pub fn run(
    target: &str,
    remove: bool,
    dry: bool,
    yes: bool,
    project: Option<&str>,
) -> Result<Value> {
    if !["all", "claude", "codex"].contains(&target) {
        return Err(Error::new(1, "usage", "invalid skill target"));
    }
    let project = project.map(std::path::absolute).transpose()?;
    if project.as_ref().is_some_and(|p| !p.is_dir()) {
        return Err(Error::new(1, "bad_project", "no such project directory"));
    }
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let mut items = Vec::new();
    let mut warnings = Vec::new();
    use std::os::unix::fs::PermissionsExt;
    if !remove
        && !std::env::var_os("PATH").is_some_and(|v| {
            std::env::split_paths(&v).any(|p| {
                p.join("corral")
                    .metadata()
                    .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            })
        })
    {
        warnings.push(
            "corral is not on PATH: the skill will tell agents that corral is not installed".into(),
        );
    }

    for agent in ["claude", "codex"] {
        if target != "all" && target != agent {
            continue;
        }
        let base = if let Some(p) = &project {
            p.join(if agent == "claude" {
                ".claude"
            } else {
                ".agents"
            })
        } else if agent == "claude" {
            std::env::var_os("CLAUDE_CONFIG_DIR")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".claude"))
        } else {
            home.join(".agents")
        };
        let path = base.join("skills/corral/SKILL.md");
        let foreign_link = linked(&path, &base);
        let current = if foreign_link {
            None
        } else {
            match fs::read_to_string(&path) {
                Ok(s) => Some(s),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e.into()),
            }
        };
        let status = if foreign_link {
            "foreign"
        } else if remove {
            match current.as_deref() {
                None => "absent",
                Some(s) if s.contains("<!-- corral-skill:") => "remove",
                Some(_) => "foreign",
            }
        } else {
            match current.as_deref() {
                None => "create",
                Some(s) if s == BODY => "same",
                Some(_) => "overwrite",
            }
        };
        if status == "foreign" {
            warnings.push(format!(
                "{} is not an owned regular skill file; left in place",
                path.display()
            ));
        }
        items.push(json!({"agent":agent,"path":path,"status":status}));
    }
    let action = if remove { "remove" } else { "install" };
    let mut result = json!({"ok":true,"action":action,"dry_run":dry,"written":false,"items":items,"warnings":warnings});
    let todo: Vec<_> = items
        .iter()
        .filter(|i| {
            matches!(
                i["status"].as_str(),
                Some("create" | "overwrite" | "remove")
            )
        })
        .collect();
    if dry || todo.is_empty() {
        return Ok(result);
    }
    if !yes {
        if !std::io::stdin().is_terminal() {
            return Err(Error::new(1,"confirmation_required","install-skills writes skill files; confirm interactively or pass --yes after user approval").with("action",action).with("items",json!(items)).with("warnings",json!(warnings)));
        }
        let verb = if remove { "删除" } else { "写入" };
        eprintln!("corral install-skills 将{verb}：");
        for item in &todo {
            eprintln!(
                "  [{}] {}",
                item["status"].as_str().unwrap(),
                item["path"].as_str().unwrap()
            );
        }
        eprint!("确认{verb}以上文件？[y/N] ");
        std::io::stderr().flush()?;
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        if !["y", "yes"].contains(&line.trim().to_lowercase().as_str()) {
            return Err(Error::new(1, "declined", "nothing written"));
        }
    }
    for item in todo {
        let path = PathBuf::from(item["path"].as_str().unwrap());
        if remove {
            fs::remove_file(&path)?;
            let _ = fs::remove_dir(path.parent().unwrap());
        } else {
            let parent = path.parent().unwrap();
            fs::create_dir_all(parent)?;
            let temp = parent.join(format!(".corral-skill-{}", uuid::Uuid::new_v4()));
            let mut f = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temp)?;
            f.write_all(BODY.as_bytes())?;
            fs::rename(temp, path)?;
        }
    }
    result["written"] = json!(true);
    Ok(result)
}
