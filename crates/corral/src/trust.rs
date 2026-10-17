//! Trusted directories for an agent that trusts only the exact directories in its own list
//! (`agents::Trust`; agy, R13). Claude Code trusts a directory under a trusted one, Codex the
//! worktrees of a trusted repository; for such an agent the pen adds its working directory to the
//! list when a parent directory is already there, and takes it out when the agent exits.
//!
//! What corral added is recorded next to the agent's HOME, in `<HOME>/.corral/.runtime/trust.json`,
//! so every corral that writes the same list (whatever its `CORRAL_HOME`) shares one record and one
//! lock (`trust.lock`). An entry is recorded as pending before its directory goes into the list and
//! confirmed after; a pending entry left by an interrupted add is dropped without touching the list,
//! so corral never takes out a directory it is not sure it added (at worst one stays listed). An
//! entry leaves the record only after its directory left the list, or the list is gone.
//!
//! The list only holds paths: a directory the user trusts again while corral's agent runs in it
//! is taken out when that agent exits.
//!
//! The agent writes its list without corral's lock. Before replacing the file corral checks it is
//! still the same file with the same text and starts over if not; what changes in between that
//! check and the replacement would be lost (the user accepted this, 10-17).
use crate::{Error, Result, state};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::{fd::AsRawFd, unix::fs::DirBuilderExt, unix::fs::OpenOptionsExt},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// Before the pen starts the agent, with the agent's HOME: trusts `cwd` for an agent of `kind` if
/// a parent is trusted. Returns the HOME whose record to clean when the agent exits.
pub fn add(
    kind: &str,
    home: &Path,
    cwd: &str,
    name: &str,
    instance: &str,
) -> Result<Option<PathBuf>> {
    let Some(t) = crate::agents::find(kind).and_then(|a| a.trust()) else {
        return Ok(None);
    };
    let owner = json!({
        "root": state::home(),
        "name": name,
        "instance": instance,
        "pen_pid": std::process::id(),
    });
    let store = store(home);
    locked(&store, || {
        add_in(&store, &(t.file)(home), t.key, Path::new(cwd), owner)
    })?;
    Ok(Some(home.to_owned()))
}

/// After the agent exited: takes out what `add` put in for this instance.
pub fn remove(home: &Path, instance: &str) -> Result<()> {
    let store = store(home);
    locked(&store, || remove_in(&store, instance))
}

/// Where the record and the lock for the lists under `home` live.
fn store(home: &Path) -> PathBuf {
    home.join(".corral/.runtime")
}

/// Runs `f` holding the exclusive lock of the record in `store`; gives up after a few seconds.
fn locked<T>(store: &Path, f: impl FnOnce() -> Result<T>) -> Result<T> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(store)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(store.join("trust.lock"))?;
    let end = Instant::now() + Duration::from_secs(5);
    while unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        let e = std::io::Error::last_os_error();
        if e.kind() != std::io::ErrorKind::WouldBlock {
            return Err(e.into());
        }
        if Instant::now() > end {
            return Err(Error::new(1, "busy", "the trust record stayed locked"));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    f()
}

fn read_record(store: &Path) -> Vec<Value> {
    state::read(store.join("trust.json"))["entries"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}
fn write_record(store: &Path, entries: &[Value]) -> Result<()> {
    state::write(&store.join("trust.json"), &json!({ "entries": entries }))
}

/// Whether a process exists.
fn exists(pid: Option<i64>) -> bool {
    pid.is_some_and(|pid| {
        pid > 0
            && (unsafe { libc::kill(pid as i32, 0) } == 0
                || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM))
    })
}

/// Whether the agent that made this record entry may still run. Its state directory names its
/// instance once it started (`corral upgrade` keeps instance and agent); while its pen is still
/// starting the agent, that directory has no metadata, and the pen has to be alive.
fn alive(entry: &Value) -> bool {
    let name = entry["name"].as_str().unwrap_or("");
    if state::validate(name).is_err() {
        return false;
    }
    let dir = Path::new(entry["root"].as_str().unwrap_or("")).join(name);
    let meta = state::read(dir.join("meta.json"));
    if meta.is_null() {
        return exists(entry["pen_pid"].as_i64());
    }
    meta["instance"] == entry["instance"] && exists(meta["agent_pid"].as_i64())
}

/// Changes the list under `key` in the JSON object file `file`: `change` gets the listed
/// directories and returns the new list, or `None` to leave it. Leaves the file alone, returning
/// `false`, when it is missing, a symbolic link, not a JSON object, or `key` holds anything but
/// strings. Starts over when the file is replaced or changes while corral writes it; `meanwhile`
/// runs in that gap, for tests.
fn update(
    file: &Path,
    key: &str,
    mut change: impl FnMut(&[String]) -> Option<Vec<String>>,
    meanwhile: &dyn Fn(),
) -> Result<bool> {
    use std::os::unix::fs::MetadataExt;
    let identity = || {
        fs::symlink_metadata(file)
            .ok()
            .filter(|m| m.file_type().is_file())
            .map(|m| (m.dev(), m.ino()))
    };
    for _ in 0..5 {
        let Some(before) = identity() else {
            return Ok(false);
        };
        let Ok(text) = fs::read_to_string(file) else {
            return Ok(false);
        };
        let Ok(mut doc) = serde_json::from_str::<Value>(&text) else {
            return Ok(false);
        };
        let paths: Vec<String> = match doc.get(key) {
            None if doc.is_object() => vec![],
            Some(Value::Array(a)) if a.iter().all(Value::is_string) => a
                .iter()
                .map(|v| v.as_str().unwrap_or_default().to_owned())
                .collect(),
            _ => return Ok(false),
        };
        let Some(paths) = change(&paths) else {
            return Ok(false);
        };
        doc[key] = json!(paths);
        let mut out = serde_json::to_string_pretty(&doc)?;
        if text.ends_with('\n') {
            out.push('\n');
        }
        let name = file.file_name().unwrap_or_default().to_string_lossy();
        let tmp = file.with_file_name(format!(".{name}.corral-{}.tmp", std::process::id()));
        let written = (|| -> Result<bool> {
            let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
            f.set_permissions(fs::metadata(file)?.permissions())?;
            f.write_all(out.as_bytes())?;
            f.sync_all()?;
            meanwhile();
            if identity() != Some(before)
                || fs::read_to_string(file).ok().as_deref() != Some(text.as_str())
            {
                return Ok(false);
            }
            fs::rename(&tmp, file)?;
            Ok(true)
        })();
        if !matches!(written, Ok(true)) {
            let _ = fs::remove_file(&tmp);
        }
        if written? {
            return Ok(true);
        }
    }
    Err(Error::new(1, "busy", "the trust list kept changing"))
}

/// Takes the directory of a record entry out of its list, unless another entry still needs it.
/// Only one copy goes: any other is not corral's. Fails when the list is there but cannot be read
/// or written safely, so that the entry stays for the next time.
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
        .any(|e| e["file"] == file && e["key"] == key && e["path"] == path && e["pending"] != true)
    {
        return Ok(());
    }
    let read = std::cell::Cell::new(false);
    let take = |paths: &[String]| {
        read.set(true);
        let at = paths.iter().position(|p| p == path)?;
        let mut paths = paths.to_vec();
        paths.remove(at);
        Some(paths)
    };
    let file = Path::new(file);
    let gone =
        || fs::symlink_metadata(file).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound);
    if update(file, key, take, &|| {})? || read.get() || gone() {
        return Ok(());
    }
    Err(Error::new(
        1,
        "error",
        format!("cannot safely change {}", file.display()),
    ))
}

/// Takes out what record entries of agents that are gone, or of directories that are gone, put in;
/// an entry whose change fails stays for the next time. A pending entry is left by an add that was
/// cut short (adds hold the lock): it is dropped, and its directory, if listed, stays.
fn prune(store: &Path) -> Result<Vec<Value>> {
    let entries = read_record(store);
    let (mut kept, stale): (Vec<Value>, Vec<Value>) = entries.iter().cloned().partition(|e| {
        e["pending"] != true && alive(e) && Path::new(e["path"].as_str().unwrap_or("")).is_dir()
    });
    for e in stale {
        if e["pending"] != true && untrust(&e, &kept).is_err() {
            kept.push(e);
        }
    }
    if kept.len() != entries.len() {
        write_record(store, &kept)?;
    }
    Ok(kept)
}

fn add_in(store: &Path, file: &Path, key: &str, cwd: &Path, owner: Value) -> Result<()> {
    let mut kept = prune(store)?;
    let dir = fs::canonicalize(cwd)?;
    let path = dir
        .to_str()
        .ok_or_else(|| Error::new(1, "error", "non UTF-8 directory"))?
        .to_owned();
    let mut entry = owner;
    entry["file"] = json!(file.to_string_lossy());
    entry["key"] = json!(key);
    entry["path"] = json!(path);
    // Another running agent of corral's already holds it: share the entry.
    if kept
        .iter()
        .any(|e| e["file"] == entry["file"] && e["key"] == key && e["path"] == entry["path"])
    {
        kept.push(entry);
        return write_record(store, &kept);
    }
    let add = |paths: &[String]| {
        let listed = paths
            .iter()
            .any(|p| Path::new(p) == dir || fs::canonicalize(p).is_ok_and(|c| c == dir));
        let under_trusted = paths.iter().any(|p| {
            let p = fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(p));
            dir != p && dir.starts_with(&p)
        });
        if listed || !under_trusted {
            return None;
        }
        let mut paths = paths.to_vec();
        paths.push(path.clone());
        Some(paths)
    };
    // Recorded as pending first, confirmed after: if adding is cut short, `prune` drops the entry
    // without taking out a directory the user may have listed.
    entry["pending"] = json!(true);
    kept.push(entry);
    write_record(store, &kept)?;
    let added = update(file, key, add, &|| {});
    let mut entry = kept.pop().unwrap_or_default();
    if matches!(added, Ok(true)) {
        entry.as_object_mut().map(|e| e.remove("pending"));
        kept.push(entry);
    }
    write_record(store, &kept)?;
    added.map(|_| ())
}

fn remove_in(store: &Path, instance: &str) -> Result<()> {
    let (mine, mut kept): (Vec<Value>, Vec<Value>) = read_record(store)
        .into_iter()
        .partition(|e| e["instance"] == instance);
    if mine.is_empty() {
        return Ok(());
    }
    let mut failed = None;
    // Out of the list first: if that fails, the record keeps the entry for the next `prune`. A
    // pending entry (its add was cut short) goes without touching the list, as in `prune`.
    for e in mine {
        if e["pending"] != true
            && let Err(err) = untrust(&e, &kept)
        {
            kept.push(e);
            failed = Some(err);
        }
    }
    write_record(store, &kept)?;
    failed.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const KEY: &str = "trustedWorkspaces";

    struct Lab {
        _tmp: tempfile::TempDir,
        base: PathBuf,
        store: PathBuf,
        file: PathBuf,
        top: PathBuf,
    }
    impl Lab {
        /// A record store, and an agent settings file trusting `top` and one other directory.
        fn new() -> Lab {
            let tmp = tempfile::tempdir().unwrap();
            let base = fs::canonicalize(tmp.path()).unwrap();
            let store = base.join("home/.corral/.runtime");
            let top = base.join("projects");
            fs::create_dir_all(&top).unwrap();
            fs::create_dir_all(base.join("settings")).unwrap();
            let file = base.join("settings/settings.json");
            let text = format!(
                "{{\n  \"model\": \"m\",\n  \"{KEY}\": [\n    \"/elsewhere\",\n    \"{}\"\n  ],\n  \"statusLine\": {{\"type\": \"command\"}}\n}}\n",
                top.display()
            );
            fs::write(&file, text).unwrap();
            fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();
            Lab {
                base,
                store,
                file,
                top,
                _tmp: tmp,
            }
        }
        /// A working directory under the trusted one.
        fn dir(&self, name: &str) -> PathBuf {
            let d = self.top.join(name);
            fs::create_dir_all(&d).unwrap();
            d
        }
        /// Marks agent `name` under corral root `root` as running `instance`, as its pen does once
        /// the agent started.
        fn running(&self, root: &str, name: &str, instance: &str) {
            let d = self.base.join(root).join(name);
            fs::create_dir_all(&d).unwrap();
            let meta = json!({"instance": instance, "agent_pid": std::process::id()});
            fs::write(d.join("meta.json"), meta.to_string()).unwrap();
        }
        /// Adds as the pen of `name` under corral root `root` would; that pen is this process.
        fn add_as(&self, root: &str, cwd: &Path, name: &str, instance: &str) -> Result<()> {
            let owner = json!({"root": self.base.join(root), "name": name, "instance": instance,
                               "pen_pid": std::process::id()});
            locked(&self.store, || {
                add_in(&self.store, &self.file, KEY, cwd, owner)
            })
        }
        fn add(&self, cwd: &Path, name: &str, instance: &str) {
            self.running("corral", name, instance);
            self.add_as("corral", cwd, name, instance).unwrap();
        }
        fn remove(&self, instance: &str) -> Result<()> {
            locked(&self.store, || remove_in(&self.store, instance))
        }
        /// Makes record entry `i` look like its pen died.
        fn kill_pen(&self, i: usize) {
            let mut record = read_record(&self.store);
            record[i]["pen_pid"] = json!(999_999_999);
            write_record(&self.store, &record).unwrap();
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
        fn with(&self, extra: &[&Path]) -> Vec<String> {
            let mut v = self.original();
            v.extend(extra.iter().map(|p| p.display().to_string()));
            v
        }
    }

    #[test]
    fn a_directory_under_a_trusted_one_is_trusted_until_its_agent_exits() {
        let lab = Lab::new();
        let work = lab.dir("work");
        lab.add(&work, "a/dev", "i1");
        assert_eq!(lab.list(), lab.with(&[&work]));
        // The rest of the file stays as it was, values and permissions alike.
        let v: Value = serde_json::from_str(&fs::read_to_string(&lab.file).unwrap()).unwrap();
        assert_eq!(
            (&v["model"], &v["statusLine"]["type"]),
            (&json!("m"), &json!("command"))
        );
        assert!(fs::read_to_string(&lab.file).unwrap().ends_with("}\n"));
        let mode = fs::metadata(&lab.file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640);
        assert_eq!(read_record(&lab.store).len(), 1);
        lab.remove("i1").unwrap();
        assert_eq!(lab.list(), lab.original());
        assert!(read_record(&lab.store).is_empty());
        // No temporary file is left next to the settings.
        let names: Vec<_> = fs::read_dir(lab.file.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names, ["settings.json"]);
    }

    #[test]
    fn what_corral_did_not_add_is_left_alone() {
        let lab = Lab::new();
        // Not under a trusted directory: the agent asks as before.
        let outside = lab.base.join("outside");
        fs::create_dir(&outside).unwrap();
        lab.add(&outside, "a/out", "i1");
        assert_eq!(lab.list(), lab.original());
        // A directory the user trusts is never taken out.
        lab.add(&lab.top, "a/top", "i2");
        lab.remove("i2").unwrap();
        assert_eq!(lab.list(), lab.original());
        assert!(read_record(&lab.store).is_empty());
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
            assert!(read_record(&lab.store).is_empty());
        }
        // No settings file at all: nothing is created.
        let lab = Lab::new();
        fs::remove_file(&lab.file).unwrap();
        lab.add(&lab.dir("work"), "a/dev", "i1");
        assert!(!lab.file.exists());
        // A settings file that links elsewhere, as dotfiles do, stays a link to an unchanged file.
        let lab = Lab::new();
        let target = lab.base.join("dotfiles.json");
        fs::rename(&lab.file, &target).unwrap();
        std::os::unix::fs::symlink(&target, &lab.file).unwrap();
        let before = fs::read_to_string(&target).unwrap();
        lab.add(&lab.dir("work"), "a/dev", "i1");
        assert!(
            fs::symlink_metadata(&lab.file)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(&target).unwrap(), before);
        assert!(read_record(&lab.store).is_empty());
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
        // Its agent died with its pen.
        fs::write(
            lab.base.join("corral/a/killed/meta.json"),
            json!({"instance": "i1", "agent_pid": 999_999_999}).to_string(),
        )
        .unwrap();
        // Its worktree was removed while the agent still runs.
        fs::remove_dir(&removed).unwrap();
        // The name now belongs to another instance.
        lab.running("corral", "a/restarted", "other");
        let new = lab.dir("new");
        lab.add(&new, "a/new", "i4");
        assert_eq!(lab.list(), lab.with(&[&new]));
        assert_eq!(read_record(&lab.store).len(), 1);
    }

    #[test]
    fn an_agent_still_starting_keeps_its_entry() {
        let lab = Lab::new();
        let first = lab.dir("first");
        // Its pen added the entry but has not written the agent's metadata yet.
        lab.add_as("corral", &first, "a/first", "i1").unwrap();
        let second = lab.dir("second");
        lab.add(&second, "a/second", "i2");
        assert_eq!(lab.list(), lab.with(&[&first, &second]));
        // A pen that died before it started the agent leaves nothing behind.
        lab.kill_pen(0);
        let third = lab.dir("third");
        lab.add(&third, "a/third", "i3");
        assert_eq!(lab.list(), lab.with(&[&second, &third]));
    }

    #[test]
    fn a_directory_two_agents_share_stays_trusted_until_both_exit() {
        let lab = Lab::new();
        let work = lab.dir("work");
        lab.add(&work, "a/one", "i1");
        lab.add(&work, "a/two", "i2");
        assert_eq!(lab.list(), lab.with(&[&work]));
        lab.remove("i1").unwrap();
        assert_eq!(lab.list(), lab.with(&[&work]));
        lab.remove("i2").unwrap();
        assert_eq!(lab.list(), lab.original());
    }

    #[test]
    fn corral_homes_sharing_one_list_share_its_record() {
        let lab = Lab::new();
        let work = lab.dir("work");
        lab.running("corral-a", "a/one", "i1");
        lab.running("corral-b", "b/one", "i2");
        lab.add_as("corral-a", &work, "a/one", "i1").unwrap();
        lab.add_as("corral-b", &work, "b/one", "i2").unwrap();
        lab.remove("i1").unwrap();
        assert_eq!(lab.list(), lab.with(&[&work]));
        lab.remove("i2").unwrap();
        assert_eq!(lab.list(), lab.original());
    }

    #[test]
    fn a_failed_change_leaves_nothing_unowned() {
        let lab = Lab::new();
        let settings = lab.file.parent().unwrap().to_owned();
        let lock = |mode| fs::set_permissions(&settings, fs::Permissions::from_mode(mode)).unwrap();
        let work = lab.dir("work");
        // Adding fails: the record does not keep a directory that is not listed.
        lab.running("corral", "a/dev", "i1");
        lock(0o500);
        assert!(lab.add_as("corral", &work, "a/dev", "i1").is_err());
        lock(0o700);
        assert_eq!(lab.list(), lab.original());
        assert!(read_record(&lab.store).is_empty());
        // Removing fails: the record keeps the entry, and a later add takes it out.
        lab.add(&work, "a/dev", "i1");
        lock(0o500);
        assert!(lab.remove("i1").is_err());
        lock(0o700);
        assert_eq!(lab.list(), lab.with(&[&work]));
        assert_eq!(read_record(&lab.store).len(), 1);
        fs::remove_dir_all(lab.base.join("corral/a/dev")).unwrap();
        lab.kill_pen(0);
        let other = lab.dir("other");
        lab.add(&other, "a/other", "i2");
        assert_eq!(lab.list(), lab.with(&[&other]));
    }

    #[test]
    fn a_change_made_while_corral_writes_is_kept() {
        let lab = Lab::new();
        let once = std::cell::Cell::new(true);
        let edit = || {
            if once.replace(false) {
                let text = fs::read_to_string(&lab.file).unwrap();
                let mut v: Value = serde_json::from_str(&text).unwrap();
                v["model"] = json!("user-new");
                v[KEY].as_array_mut().unwrap().push(json!("/user-added"));
                fs::write(&lab.file, v.to_string()).unwrap();
            }
        };
        let add = |paths: &[String]| {
            let mut p = paths.to_vec();
            p.push("/corral-added".into());
            Some(p)
        };
        assert!(update(&lab.file, KEY, add, &edit).unwrap());
        let v: Value = serde_json::from_str(&fs::read_to_string(&lab.file).unwrap()).unwrap();
        assert_eq!(v["model"], "user-new");
        let mut want = lab.original();
        want.extend(["/user-added".to_owned(), "/corral-added".to_owned()]);
        assert_eq!(lab.list(), want);
    }

    #[test]
    fn an_add_cut_short_never_takes_out_a_listed_directory() {
        let lab = Lab::new();
        let work = lab.dir("work");
        // The user trusts `work`; a pen recorded it as pending and died before it read the list.
        fs::write(
            &lab.file,
            json!({ KEY: [lab.top.clone(), work.clone()] }).to_string(),
        )
        .unwrap();
        let owner = json!({"root": lab.base.join("corral"), "name": "a/dead", "instance": "i1",
                           "pen_pid": 999_999_999, "file": lab.file, "key": KEY,
                           "path": work, "pending": true});
        fs::create_dir_all(&lab.store).unwrap();
        write_record(&lab.store, &[owner]).unwrap();
        lab.add(&lab.dir("other"), "a/other", "i2");
        assert!(lab.list().contains(&work.display().to_string()));
        assert_eq!(read_record(&lab.store).len(), 1);
        // A confirmed entry has no pending mark.
        assert!(read_record(&lab.store)[0].get("pending").is_none());
    }

    #[test]
    fn a_list_corral_cannot_read_keeps_the_record_until_it_can() {
        let lab = Lab::new();
        let work = lab.dir("work");
        lab.add(&work, "a/dev", "i1");
        let mode = fs::metadata(&lab.file).unwrap().permissions();
        fs::set_permissions(&lab.file, fs::Permissions::from_mode(0o000)).unwrap();
        assert!(lab.remove("i1").is_err());
        assert_eq!(read_record(&lab.store).len(), 1);
        fs::set_permissions(&lab.file, mode).unwrap();
        // Turned into a link: still kept, and the link is not replaced.
        let target = lab.base.join("dotfiles.json");
        fs::rename(&lab.file, &target).unwrap();
        std::os::unix::fs::symlink(&target, &lab.file).unwrap();
        assert!(lab.remove("i1").is_err());
        assert_eq!(read_record(&lab.store).len(), 1);
        fs::remove_file(&lab.file).unwrap();
        fs::rename(&target, &lab.file).unwrap();
        lab.remove("i1").unwrap();
        assert_eq!(lab.list(), lab.original());
        assert!(read_record(&lab.store).is_empty());
        // A list that is gone has nothing left to take out.
        lab.add(&work, "a/dev", "i2");
        fs::remove_file(&lab.file).unwrap();
        lab.remove("i2").unwrap();
        assert!(read_record(&lab.store).is_empty());
    }

    #[test]
    fn a_file_replaced_while_corral_writes_is_not_overwritten() {
        let lab = Lab::new();
        let target = lab.base.join("dotfiles.json");
        // Replaced by a link to the same text: the link stays.
        let to_link = || {
            if !fs::symlink_metadata(&lab.file).unwrap().is_symlink() {
                fs::rename(&lab.file, &target).unwrap();
                std::os::unix::fs::symlink(&target, &lab.file).unwrap();
            }
        };
        let add = |paths: &[String]| {
            let mut p = paths.to_vec();
            p.push("/corral-added".into());
            Some(p)
        };
        assert!(!update(&lab.file, KEY, add, &to_link).unwrap());
        assert!(fs::symlink_metadata(&lab.file).unwrap().is_symlink());
        assert!(
            !fs::read_to_string(&target)
                .unwrap()
                .contains("corral-added")
        );
        // Replaced once by another file with the same text: corral starts over on the new one.
        let lab = Lab::new();
        let once = std::cell::Cell::new(true);
        let replace = || {
            if once.replace(false) {
                let text = fs::read_to_string(&lab.file).unwrap();
                let new = lab.base.join("new.json");
                fs::write(&new, text).unwrap();
                fs::rename(&new, &lab.file).unwrap();
            }
        };
        assert!(update(&lab.file, KEY, add, &replace).unwrap());
        assert!(lab.list().contains(&"/corral-added".to_owned()));
    }

    #[test]
    fn concurrent_pens_lose_no_entry() {
        let lab = Lab::new();
        let n = 12;
        let dirs: Vec<PathBuf> = (0..n).map(|i| lab.dir(&format!("w{i}"))).collect();
        for i in 0..n {
            lab.running("corral", &format!("a/p{i}"), &format!("i{i}"));
        }
        std::thread::scope(|s| {
            for (i, d) in dirs.iter().enumerate() {
                let lab = &lab;
                s.spawn(move || {
                    lab.add_as("corral", d, &format!("a/p{i}"), &format!("i{i}"))
                        .unwrap()
                });
            }
        });
        assert_eq!(lab.list().len(), 2 + n);
        assert_eq!(read_record(&lab.store).len(), n);
        std::thread::scope(|s| {
            for i in 0..n {
                let lab = &lab;
                s.spawn(move || lab.remove(&format!("i{i}")).unwrap());
            }
        });
        assert_eq!(lab.list(), lab.original());
    }
}
