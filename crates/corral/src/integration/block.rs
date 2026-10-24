//! Ownership and atomic edits for a marked block in a shared configuration file.
use super::{Global, Plan};
use crate::{Error, Result, skills};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};

pub(super) struct File {
    bytes: Vec<u8>,
    meta: fs::Metadata,
    range: Option<std::ops::Range<usize>>,
}

fn read(path: &Path) -> std::io::Result<(Vec<u8>, fs::Metadata)> {
    let mut f = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let meta = f.metadata()?;
    if !meta.is_file() {
        return Err(std::io::Error::other("configuration is not a regular file"));
    }
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes)?;
    Ok((bytes, meta))
}

pub(super) fn plan(g: &Global, home: &Path, remove: bool) -> Result<Plan> {
    let dir = (g.dir)(home);
    let path = dir.join(g.path);
    let mut p = Plan {
        dir,
        path,
        status: "foreign",
        version: None,
        present: vec![],
        temps: vec![],
        block: None,
    };
    if skills::linked(&p.path, &p.dir) {
        return Ok(p);
    }
    let (bytes, meta) = match read(&p.path) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            p.status = if remove { "absent" } else { "create" };
            return Ok(p);
        }
        Err(_) => return Ok(p),
    };
    let (open, close) = g.block.unwrap();
    let occurrences = |mark: &str| {
        bytes
            .windows(mark.len())
            .filter(|s| *s == mark.as_bytes())
            .count()
    };
    let body = g.files[0].1.as_bytes();
    let range = match (occurrences(open), occurrences(close)) {
        (0, 0) => None,
        (1, 1) => match bytes.windows(body.len()).position(|s| s == body) {
            Some(start) => Some(start..start + body.len()),
            None => return Ok(p),
        },
        _ => return Ok(p),
    };
    p.status = match (range.is_some(), remove) {
        (true, true) => "remove",
        (true, false) => "same",
        (false, true) => "absent",
        (false, false) => "create",
    };
    p.version = range
        .as_ref()
        .and_then(|_| super::installed_version(g.files[0].1));
    p.block = Some(File { bytes, meta, range });
    Ok(p)
}

/// Recheck identity and all bytes immediately before replacing, including --yes installs.
pub(super) fn apply(g: &Global, p: &Plan, remove: bool) -> Result<()> {
    let mut bytes = p
        .block
        .as_ref()
        .map(|f| f.bytes.clone())
        .unwrap_or_default();
    if remove {
        let mut range = p
            .block
            .as_ref()
            .and_then(|f| f.range.clone())
            .ok_or_else(|| Error::new(1, "changed", "marked block disappeared; nothing written"))?;
        if range.start > 0 && bytes[range.start - 1] != b'\n' && range.end < bytes.len() {
            // This newline also separates the user's lines on either side of the block.
            range.start += 1;
        }
        bytes.drain(range);
    } else {
        bytes.extend_from_slice(g.files[0].1.as_bytes());
    }
    let parent = p.path.parent().unwrap();
    fs::create_dir_all(parent)?;
    let temp = parent.join(format!(".corral-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut f = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&temp)?;
        f.write_all(&bytes)?;
        if let Some(old) = &p.block {
            f.set_permissions(old.meta.permissions())?;
        }
        f.sync_all()?;
        if skills::linked(&p.path, &p.dir) {
            return Err(Error::new(
                1,
                "changed",
                "configuration became a symbolic link; nothing written",
            ));
        }
        if let Some(old) = &p.block {
            let same = read(&p.path).is_ok_and(|(now, meta)| {
                now == old.bytes
                    && meta.dev() == old.meta.dev()
                    && meta.ino() == old.meta.ino()
                    && meta.mode() == old.meta.mode()
            });
            if !same {
                return Err(Error::new(
                    1,
                    "changed",
                    "configuration changed; nothing written",
                ));
            }
            fs::rename(&temp, &p.path)?;
        } else {
            fs::hard_link(&temp, &p.path)?;
        }
        Ok(())
    })();
    let _ = fs::remove_file(temp);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shared_file_changed_after_planning_is_never_written() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().canonicalize().unwrap();
        let mut g = crate::agents::find("kimi").unwrap().global().unwrap();
        g.dir = |home| home.to_path_buf();
        let path = home.join("config.toml");
        for remove in [false, true] {
            let original = if remove { g.files[0].1 } else { "# user\n" };
            fs::write(&path, original).unwrap();
            let p = plan(&g, &home, remove).unwrap();
            let changed = format!("{original}# changed by editor\n");
            fs::write(&path, &changed).unwrap();
            assert_eq!(
                apply(&g, &p, remove).unwrap_err().value["message"],
                "configuration changed; nothing written"
            );
            assert_eq!(fs::read_to_string(&path).unwrap(), changed);
            let p = plan(&g, &home, remove).unwrap();
            fs::write(home.join("replacement"), &changed).unwrap();
            fs::rename(home.join("replacement"), &path).unwrap();
            assert_eq!(
                apply(&g, &p, remove).unwrap_err().value["message"],
                "configuration changed; nothing written"
            );
            assert_eq!(fs::read_to_string(&path).unwrap(), changed);
            let p = plan(&g, &home, remove).unwrap();
            fs::remove_file(&path).unwrap();
            std::os::unix::fs::symlink(home.join("other"), &path).unwrap();
            fs::write(home.join("other"), &changed).unwrap();
            assert_eq!(
                apply(&g, &p, remove).unwrap_err().value["message"],
                "configuration became a symbolic link; nothing written"
            );
            assert!(fs::symlink_metadata(&path).unwrap().is_symlink());
            assert_eq!(fs::read_to_string(home.join("other")).unwrap(), changed);
            fs::remove_file(&path).unwrap();
        }
    }

    #[test]
    fn shared_file_edits_keep_permissions_and_use_the_planned_directory() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().canonicalize().unwrap();
        fs::create_dir(home.join("real")).unwrap();
        symlink(home.join("real"), home.join("alias")).unwrap();
        // A linked ancestor above the configuration directory is outside our ownership check.
        let dir = home.join("alias/config");
        fs::create_dir(&dir).unwrap();
        let mut g = crate::agents::find("kimi").unwrap().global().unwrap();
        g.dir = |home| home.to_path_buf();
        let path = dir.join("config.toml");
        fs::write(&path, "# user\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        for remove in [false, true] {
            let p = plan(&g, &dir, remove).unwrap();
            assert_eq!(p.status, if remove { "remove" } else { "create" });
            apply(&g, &p, remove).unwrap();
            assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o640);
        }
        assert_eq!(fs::read_to_string(path).unwrap(), "# user\n");
    }
}
