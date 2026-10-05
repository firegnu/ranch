// From Saddle `crates/corral-core/src/state.rs` at commit `a31dea2`, unchanged.
use crate::{Error, Result};
use serde_json::{Value, json};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
            net::UnixStream,
        },
    },
    path::{Path, PathBuf},
    time::Duration,
};

pub const FILES: &[&str] = &[
    "lock",
    "meta.json",
    "sock",
    "hook.py",
    "hook_pi.ts",
    "hook_omp.ts",
    "events",
    "cursor",
    "exit.json",
    "pen.log",
    "labels.json",
    "upgrade.json",
    "upgrade.committed",
    "upgrade.active",
];
pub fn home() -> PathBuf {
    std::env::var_os("CORRAL_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".corral")
        })
}
pub fn validate(name: &str) -> Result<()> {
    if name.split('/').any(|s| {
        s.is_empty()
            || !s.as_bytes()[0].is_ascii_alphanumeric()
            || FILES.contains(&s)
            || !s
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    }) {
        return Err(Error::new(1, "bad_name", format!("bad agent name: {name}")));
    }
    Ok(())
}
pub fn dir(name: &str) -> Result<PathBuf> {
    validate(name)?;
    Ok(home().join(name))
}
pub fn socket(name: &str) -> Result<PathBuf> {
    let p = dir(name)?.join("sock");
    let limit = if cfg!(target_os = "macos") { 104 } else { 108 };
    if p.as_os_str().len() >= limit {
        return Err(Error::new(1, "path_too_long", "socket path is too long"));
    }
    Ok(p)
}
pub fn read(path: impl AsRef<Path>) -> Value {
    fs::read(path)
        .ok()
        .and_then(|s| serde_json::from_slice(&s).ok())
        .unwrap_or(Value::Null)
}
pub fn write(path: &Path, v: &Value) -> Result<()> {
    write_record(path, v, false)
}
pub(crate) fn write_durable(path: &Path, v: &Value) -> Result<()> {
    write_record(path, v, true)
}
fn write_record(path: &Path, v: &Value, durable: bool) -> Result<()> {
    let tmp = path.with_file_name(format!(
        "{}.{}.tmp",
        path.file_name().unwrap().to_string_lossy(),
        std::process::id()
    ));
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)?;
    f.set_permissions(fs::Permissions::from_mode(0o600))?;
    f.write_all(&serde_json::to_vec(v)?)?;
    if durable {
        f.sync_all()?;
    }
    fs::rename(tmp, path)?;
    if durable {
        File::open(path.parent().unwrap())?.sync_all()?;
    }
    Ok(())
}
pub fn lock(path: &Path, create: bool) -> Result<Option<File>> {
    loop {
        let f = match OpenOptions::new()
            .read(true)
            .write(true)
            .create(create)
            .truncate(false)
            .mode(0o600)
            .open(path)
        {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && !create => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() == std::io::ErrorKind::WouldBlock {
                return Ok(None);
            }
            return Err(e.into());
        }
        if fs::metadata(path).is_ok_and(|m| m.ino() == f.metadata().unwrap().ino()) {
            return Ok(Some(f));
        }
    }
}
pub fn acquire(name: &str, unique: bool) -> Result<(String, File)> {
    let mut n = 1;
    loop {
        let name = if unique {
            format!("{name}-{n}")
        } else {
            name.to_owned()
        };
        socket(&name)?;
        let d = dir(&name)?;
        let mut p = home();
        let mut levels = vec![p.clone()];
        for component in name.split('/') {
            p.push(component);
            levels.push(p.clone());
        }
        for p in levels {
            match fs::DirBuilder::new().mode(0o700).create(&p) {
                Ok(()) => fs::set_permissions(p, fs::Permissions::from_mode(0o700))?,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && p.is_dir() => {}
                Err(e) => return Err(e.into()),
            }
        }
        if let Some(f) = lock(&d.join("lock"), true)? {
            return Ok((name, f));
        }
        if !unique {
            return Err(Error::new(
                5,
                "exists",
                format!("{name} is already running"),
            ));
        }
        n += 1;
    }
}
pub fn not_found(name: &str) -> Error {
    Error::new(2, "not_found", format!("{name} is not running"))
        .with("name", name)
        .with(
            "exited",
            dir(name)
                .map(|d| read(d.join("exit.json")))
                .unwrap_or(Value::Null),
        )
}
pub fn check_proto(v: &Value) -> Result<()> {
    if v != &json!(1) {
        return Err(
            Error::new(9, "incompatible", "unsupported pen protocol").with("proto", v.clone())
        );
    }
    Ok(())
}
pub fn request(name: &str, req: Value) -> Result<Value> {
    request_timeout(name, req, Duration::from_secs(30))
}
pub(crate) fn request_timeout(name: &str, mut req: Value, timeout: Duration) -> Result<Value> {
    let mut sock = UnixStream::connect(socket(name)?).map_err(|_| not_found(name))?;
    let meta = read(dir(name)?.join("meta.json"));
    if let Some(v) = meta.get("proto") {
        check_proto(v)?;
    }
    sock.set_read_timeout(Some(timeout))?;
    sock.set_write_timeout(Some(timeout))?;
    req["proto"] = json!(1);
    sock.write_all(&serde_json::to_vec(&req)?)
        .map_err(|_| not_found(name))?;
    sock.write_all(b"\n").map_err(|_| not_found(name))?;
    let mut buf = Vec::new();
    let mut r = BufReader::new(sock);
    // Cap untrusted replies at the v1 protocol limit.
    while buf.len() <= 16 * 1024 * 1024 {
        let bytes = r.fill_buf().map_err(|_| not_found(name))?;
        if bytes.is_empty() {
            break;
        }
        let n = bytes
            .iter()
            .position(|b| *b == b'\n')
            .map_or(bytes.len(), |n| n + 1);
        buf.extend_from_slice(&bytes[..n]);
        r.consume(n);
        if buf.last() == Some(&b'\n') {
            break;
        }
    }
    if buf.is_empty() {
        return Err(not_found(name));
    }
    if buf.len() > 16 * 1024 * 1024 {
        return Err(Error::new(1, "bad_reply", "reply too long"));
    }
    let result: Value = serde_json::from_slice(&buf)
        .map_err(|_| Error::new(1, "bad_reply", "malformed pen reply"))?;
    check_proto(&result["proto"])?;
    Ok(result)
}
pub fn require(name: &str, req: Value) -> Result<Value> {
    let v = request(name, req)?;
    if v["ok"] != true {
        return Err(Error::new(
            1,
            v["error"].as_str().unwrap_or("pen_error"),
            v["message"].as_str().unwrap_or("pen error"),
        )
        .with("name", name));
    }
    Ok(v)
}
pub fn list() -> Result<Vec<Value>> {
    fn visit(p: &Path, root: &Path, out: &mut Vec<String>) -> std::io::Result<()> {
        if p != root && p.join("lock").is_file() {
            out.push(p.strip_prefix(root).unwrap().to_string_lossy().into_owned())
        }
        for e in fs::read_dir(p)? {
            let e = e?;
            if e.file_type()?.is_dir() {
                visit(&e.path(), root, out)?
            }
        }
        Ok(())
    }
    let root = home();
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    visit(&root, &root, &mut names)?;
    names.sort();
    let mut agents = Vec::new();
    for name in names {
        match request_timeout(&name, json!({"op":"status"}), Duration::from_secs(5)) {
            Ok(st) if st["ok"] == true => {
                let d = dir(&name)?;
                let m = read(d.join("meta.json"));
                let l = read(d.join("labels.json"));
                let labels = if l["instance"] == st["instance"] && l["labels"].is_object() {
                    l["labels"].clone()
                } else {
                    json!({})
                };
                agents.push(json!({"name":name,"instance":st["instance"],"kind":m["kind"],"cwd":m["cwd"],"started":st["started"],"labels":labels}));
            }
            Err(e) if e.code == 9 => {
                agents.push(json!({"name":name,"incompatible":true,"proto":e.value["proto"]}))
            }
            Ok(_) | Err(_) => {
                let d = dir(&name)?;
                if let Some(_lock) = lock(&d.join("lock"), false)? {
                    for f in FILES {
                        if *f != "lock" {
                            let _ = fs::remove_file(d.join(f));
                        }
                    }
                    let _ = fs::remove_file(d.join("lock"));
                    let mut p = d.as_path();
                    while p != root {
                        if fs::remove_dir(p).is_err() {
                            break;
                        }
                        p = p.parent().unwrap();
                    }
                } else if d.join("lock").exists() {
                    agents.push(json!({"name":name,"starting":true}));
                }
            }
        }
    }
    Ok(agents)
}
