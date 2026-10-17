//! Trusted directories for an agent that trusts only the exact directories in its own list
//! (`agents::Trust`; agy, R13). Claude Code trusts a directory under a trusted one, Codex the
//! worktrees of a trusted repository; for such an agent the pen adds its working directory to the
//! list when a parent directory is already there, and takes it out when the agent exits.
//!
//! What corral added is recorded in `$CORRAL_HOME/.runtime/trust.json`, so corral only ever
//! removes its own entries. Adding and removing happen under `.runtime/trust.lock`.
use crate::{Error, Result, state};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::{fd::AsRawFd, unix::fs::DirBuilderExt, unix::fs::OpenOptionsExt},
    path::{Path, PathBuf},
};

/// Before the pen starts the agent: trusts `cwd` for an agent of `kind` if a parent is trusted.
pub fn add(kind: &str, cwd: &str, name: &str, instance: &str) -> Result<()> {
    let Some(t) = crate::agents::find(kind).and_then(|a| a.trust()) else {
        return Ok(());
    };
    let file = (t.file)(&home());
    locked(&state::home(), |root| {
        add_in(root, &file, t.key, Path::new(cwd), name, instance)
    })
}

/// After the agent exited: takes out what `add` put in for this instance.
pub fn remove(kind: &str, instance: &str) -> Result<()> {
    if crate::agents::find(kind).and_then(|a| a.trust()).is_none() {
        return Ok(());
    }
    locked(&state::home(), |root| remove_in(root, instance))
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

/// Runs `f` holding the exclusive lock of `root`'s trust record.
fn locked<T>(root: &Path, f: impl FnOnce(&Path) -> Result<T>) -> Result<T> {
    let runtime = root.join(".runtime");
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&runtime)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(runtime.join("trust.lock"))?;
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    f(root)
}

fn record_path(root: &Path) -> PathBuf {
    root.join(".runtime/trust.json")
}
fn read_record(root: &Path) -> Vec<Value> {
    state::read(record_path(root))["entries"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}
fn write_record(root: &Path, entries: Vec<Value>) -> Result<()> {
    state::write(&record_path(root), &json!({"entries": entries}))
}

/// Whether the agent that made this record entry still runs: its state directory names the same
/// instance and its process exists.
fn alive(root: &Path, entry: &Value) -> bool {
    let name = entry["name"].as_str().unwrap_or("");
    if state::validate(name).is_err() {
        return false;
    }
    let meta = state::read(root.join(name).join("meta.json"));
    let Some(pid) = meta["agent_pid"].as_i64() else {
        return false;
    };
    meta["instance"] == entry["instance"]
        && (unsafe { libc::kill(pid as i32, 0) } == 0
            || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM))
}

/// The agent's trust list in `file`, or `None` when corral must leave the file alone: it is
/// missing, not a JSON object, or `key` is not a list of strings. A missing `key` is an empty list.
struct List {
    file: PathBuf,
    doc: Value,
    key: String,
    newline: bool,
}
impl List {
    fn read(file: &Path, key: &str) -> Option<List> {
        let text = fs::read_to_string(file).ok()?;
        let doc: Value = serde_json::from_str(&text).ok()?;
        match doc.get(key) {
            None if doc.is_object() => {}
            Some(Value::Array(a)) if a.iter().all(Value::is_string) => {}
            _ => return None,
        }
        Some(List {
            file: file.to_owned(),
            doc,
            key: key.to_owned(),
            newline: text.ends_with('\n'),
        })
    }
    fn paths(&self) -> Vec<String> {
        self.doc[&self.key]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }
    fn set(&mut self, paths: Vec<String>) {
        self.doc[&self.key] = json!(paths);
    }
    /// Replaces the file atomically, keeping its permissions.
    fn write(&self) -> Result<()> {
        let mode = fs::metadata(&self.file)?.permissions();
        let name = self.file.file_name().unwrap_or_default().to_string_lossy();
        let tmp = self
            .file
            .with_file_name(format!(".{name}.corral-{}.tmp", std::process::id()));
        let mut text = serde_json::to_string_pretty(&self.doc)?;
        if self.newline {
            text.push('\n');
        }
        let result = (|| -> Result<()> {
            let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
            f.set_permissions(mode)?;
            f.write_all(text.as_bytes())?;
            f.sync_all()?;
            fs::rename(&tmp, &self.file)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result
    }
}

/// Takes the directory of a record entry out of its list, unless another entry still needs it.
fn untrust(entry: &Value, kept: &[Value]) -> Result<()> {
    let (Some(file), Some(key), Some(path)) = (
        entry["file"].as_str(),
        entry["key"].as_str(),
        entry["path"].as_str(),
    ) else {
        return Ok(());
    };
    if kept
        .iter()
        .any(|e| e["file"] == file && e["key"] == key && e["path"] == path)
    {
        return Ok(());
    }
    let Some(mut list) = List::read(Path::new(file), key) else {
        return Ok(());
    };
    let paths = list.paths();
    if !paths.iter().any(|p| p == path) {
        return Ok(());
    }
    list.set(paths.into_iter().filter(|p| p != path).collect());
    list.write()
}

fn add_in(
    root: &Path,
    file: &Path,
    key: &str,
    cwd: &Path,
    name: &str,
    instance: &str,
) -> Result<()> {
    // First drop what agents that are gone, or directories that are gone, left behind.
    let (mut kept, stale): (Vec<Value>, Vec<Value>) = read_record(root)
        .into_iter()
        .partition(|e| alive(root, e) && Path::new(e["path"].as_str().unwrap_or("")).is_dir());
    for e in &stale {
        untrust(e, &kept)?;
    }
    if !stale.is_empty() {
        write_record(root, kept.clone())?;
    }

    let Some(mut list) = List::read(file, key) else {
        return Ok(());
    };
    let dir = fs::canonicalize(cwd)?;
    let path = dir
        .to_str()
        .ok_or_else(|| Error::new(1, "error", "non UTF-8 directory"))?
        .to_owned();
    let file_str = file.to_string_lossy().into_owned();
    let paths = list.paths();
    let listed = paths
        .iter()
        .any(|p| Path::new(p) == dir || fs::canonicalize(p).is_ok_and(|c| c == dir));
    let ours = kept
        .iter()
        .any(|e| e["file"] == file_str.as_str() && e["key"] == key && e["path"] == path.as_str());
    if listed && !ours {
        // The user trusts it already.
        return Ok(());
    }
    if !listed {
        let parent_trusted = paths.iter().any(|p| {
            let p = fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(p));
            dir != p && dir.starts_with(&p)
        });
        if !parent_trusted {
            return Ok(());
        }
        let mut paths = paths;
        paths.push(path.clone());
        list.set(paths);
        list.write()?;
    }
    kept.push(
        json!({"file": file_str, "key": key, "path": path, "name": name, "instance": instance}),
    );
    write_record(root, kept)
}

fn remove_in(root: &Path, instance: &str) -> Result<()> {
    let (mine, kept): (Vec<Value>, Vec<Value>) = read_record(root)
        .into_iter()
        .partition(|e| e["instance"] == instance);
    if mine.is_empty() {
        return Ok(());
    }
    write_record(root, kept.clone())?;
    for e in &mine {
        untrust(e, &kept)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const KEY: &str = "trustedWorkspaces";

    struct Lab {
        _tmp: tempfile::TempDir,
        root: PathBuf,
        file: PathBuf,
        top: PathBuf,
    }
    impl Lab {
        /// A corral home, and an agent settings file trusting `top` and one other directory.
        fn new() -> Lab {
            let tmp = tempfile::tempdir().unwrap();
            let base = fs::canonicalize(tmp.path()).unwrap();
            let root = base.join("corral");
            let top = base.join("projects");
            fs::create_dir_all(&root).unwrap();
            fs::create_dir_all(&top).unwrap();
            let file = base.join("settings.json");
            let text = format!(
                "{{\n  \"model\": \"m\",\n  \"{KEY}\": [\n    \"/elsewhere\",\n    \"{}\"\n  ],\n  \"statusLine\": {{\"type\": \"command\"}}\n}}\n",
                top.display()
            );
            fs::write(&file, text).unwrap();
            fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();
            Lab {
                _tmp: tmp,
                root,
                file,
                top,
            }
        }
        /// A working directory under the trusted one.
        fn dir(&self, name: &str) -> PathBuf {
            let d = self.top.join(name);
            fs::create_dir_all(&d).unwrap();
            d
        }
        /// Marks agent `name` as running `instance`, as its pen would.
        fn running(&self, name: &str, instance: &str) {
            let d = self.root.join(name);
            fs::create_dir_all(&d).unwrap();
            let meta = json!({"instance": instance, "agent_pid": std::process::id()});
            fs::write(d.join("meta.json"), meta.to_string()).unwrap();
        }
        fn add(&self, cwd: &Path, name: &str, instance: &str) {
            self.running(name, instance);
            locked(&self.root, |r| {
                add_in(r, &self.file, KEY, cwd, name, instance)
            })
            .unwrap();
        }
        fn remove(&self, instance: &str) {
            locked(&self.root, |r| remove_in(r, instance)).unwrap();
        }
        fn list(&self) -> Vec<String> {
            let v: Value = serde_json::from_str(&fs::read_to_string(&self.file).unwrap()).unwrap();
            v[KEY]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p.as_str().unwrap().to_owned())
                .collect()
        }
        fn original(&self) -> Vec<String> {
            vec!["/elsewhere".into(), self.top.display().to_string()]
        }
    }

    #[test]
    fn a_directory_under_a_trusted_one_is_trusted_until_its_agent_exits() {
        let lab = Lab::new();
        let work = lab.dir("work");
        lab.add(&work, "a/dev", "i1");
        let mut want = lab.original();
        want.push(work.display().to_string());
        assert_eq!(lab.list(), want);
        // The rest of the file stays as it was, values and permissions alike.
        let v: Value = serde_json::from_str(&fs::read_to_string(&lab.file).unwrap()).unwrap();
        assert_eq!(
            (&v["model"], &v["statusLine"]["type"]),
            (&json!("m"), &json!("command"))
        );
        assert!(fs::read_to_string(&lab.file).unwrap().ends_with("}\n"));
        let mode = fs::metadata(&lab.file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640);
        assert_eq!(read_record(&lab.root).len(), 1);
        lab.remove("i1");
        assert_eq!(lab.list(), lab.original());
        assert!(read_record(&lab.root).is_empty());
        // No temporary file is left next to the settings.
        let names: Vec<_> = fs::read_dir(lab.file.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert!(names.iter().all(|n| !n.ends_with(".tmp")), "{names:?}");
    }

    #[test]
    fn what_corral_did_not_add_is_left_alone() {
        let lab = Lab::new();
        // Not under a trusted directory: the agent asks as before.
        let outside = lab.top.parent().unwrap().join("outside");
        fs::create_dir(&outside).unwrap();
        lab.add(&outside, "a/out", "i1");
        assert_eq!(lab.list(), lab.original());
        // A directory the user trusts is never taken out.
        lab.add(&lab.top, "a/top", "i2");
        lab.remove("i2");
        assert_eq!(lab.list(), lab.original());
        assert!(read_record(&lab.root).is_empty());
    }

    #[test]
    fn a_file_corral_cannot_read_safely_is_not_written() {
        for text in [
            "not json".to_owned(),
            "[]".to_owned(),
            format!("{{\"{KEY}\": \"/\"}}"),
            format!("{{\"{KEY}\": [\"/\", 1]}}"),
        ] {
            let lab = Lab::new();
            fs::write(&lab.file, &text).unwrap();
            lab.add(&lab.dir("work"), "a/dev", "i1");
            assert_eq!(fs::read_to_string(&lab.file).unwrap(), text);
            assert!(read_record(&lab.root).is_empty());
        }
        // No settings file at all: nothing is created.
        let lab = Lab::new();
        fs::remove_file(&lab.file).unwrap();
        lab.add(&lab.dir("work"), "a/dev", "i1");
        assert!(!lab.file.exists());
    }

    #[test]
    fn entries_left_by_agents_or_directories_that_are_gone_are_cleared_on_the_next_add() {
        let lab = Lab::new();
        let killed = lab.dir("killed");
        let removed = lab.dir("removed");
        let restarted = lab.dir("restarted");
        lab.add(&killed, "a/killed", "i1");
        lab.add(&removed, "a/removed", "i2");
        lab.add(&restarted, "a/restarted", "i3");
        // Its pen was killed: the agent is gone.
        fs::remove_dir_all(lab.root.join("a/killed")).unwrap();
        // Its worktree was removed while the agent still runs.
        fs::remove_dir(&removed).unwrap();
        // The name now belongs to another instance.
        lab.running("a/restarted", "other");
        lab.add(&lab.dir("new"), "a/new", "i4");
        let mut want = lab.original();
        want.push(lab.top.join("new").display().to_string());
        assert_eq!(lab.list(), want);
        assert_eq!(read_record(&lab.root).len(), 1);
    }

    #[test]
    fn a_directory_two_agents_share_stays_trusted_until_both_exit() {
        let lab = Lab::new();
        let work = lab.dir("work");
        lab.add(&work, "a/one", "i1");
        lab.add(&work, "a/two", "i2");
        let mut want = lab.original();
        want.push(work.display().to_string());
        assert_eq!(lab.list(), want);
        lab.remove("i1");
        assert_eq!(lab.list(), want);
        lab.remove("i2");
        assert_eq!(lab.list(), lab.original());
    }

    #[test]
    fn concurrent_pens_lose_no_entry() {
        let lab = Lab::new();
        let n = 12;
        let dirs: Vec<PathBuf> = (0..n).map(|i| lab.dir(&format!("w{i}"))).collect();
        for i in 0..n {
            lab.running(&format!("a/p{i}"), &format!("i{i}"));
        }
        std::thread::scope(|s| {
            for (i, d) in dirs.iter().enumerate() {
                let lab = &lab;
                s.spawn(move || {
                    let (name, inst) = (format!("a/p{i}"), format!("i{i}"));
                    locked(&lab.root, |r| add_in(r, &lab.file, KEY, d, &name, &inst)).unwrap();
                });
            }
        });
        assert_eq!(lab.list().len(), 2 + n);
        assert_eq!(read_record(&lab.root).len(), n);
        std::thread::scope(|s| {
            for i in 0..n {
                let lab = &lab;
                s.spawn(move || lab.remove(&format!("i{i}")));
            }
        });
        assert_eq!(lab.list(), lab.original());
    }
}
