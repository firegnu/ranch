//! The snapshot owns no descriptors until validation and the active boundary.
//! In particular, failed deserialization must not close the only PTY or name lock.
use super::*;
use std::{
    collections::BTreeSet,
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
    os::unix::fs::MetadataExt,
    path::Path,
    time::{Duration, Instant},
};

pub const SCHEMA: u32 = 1;

pub(super) struct Handle<T> {
    value: ManuallyDrop<T>,
    owned: bool,
}
impl<T> Handle<T> {
    pub(super) fn new(value: T) -> Self {
        Self {
            value: ManuallyDrop::new(value),
            owned: true,
        }
    }
}
impl<T> Deref for Handle<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}
impl<T> DerefMut for Handle<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}
impl<T> Drop for Handle<T> {
    fn drop(&mut self) {
        if self.owned {
            unsafe {
                ManuallyDrop::drop(&mut self.value);
            }
        }
    }
}
impl<T: AsRawFd> Serialize for Handle<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        self.as_raw_fd().serialize(s)
    }
}
impl<'de, T: FromRawFd> Deserialize<'de> for Handle<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let fd = i32::deserialize(d)?;
        if fd < 3 {
            return Err(serde::de::Error::custom("invalid inherited fd"));
        }
        Ok(Self {
            value: ManuallyDrop::new(unsafe { T::from_raw_fd(fd) }),
            owned: false,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Custody {
    Owner,
    Handoff,
    Observer,
}

#[derive(Default, Serialize, Deserialize)]
pub(super) struct Upgrade {
    epoch: Option<String>,
    attempt: u32,
    target: Option<PathBuf>,
    old: Option<PathBuf>,
    state: String,
    result: Option<String>,
    last_error: Option<String>,
    backup_pid: Option<i32>,
    control: Option<Handle<File>>,
    alive: Option<Handle<File>>,
}
impl Upgrade {
    pub(super) fn public(&self) -> Value {
        json!({"state":if self.state.is_empty(){"none"}else{&self.state},"epoch":self.epoch,"attempt":self.attempt,"target":self.target,"result":self.result,"last_error":self.last_error,"protected":self.alive.is_some(),"backup_pid":self.backup_pid})
    }
}

#[derive(Serialize, Deserialize)]
struct Descriptor {
    fd: i32,
    dev: u64,
    ino: u64,
    mode: u32,
}
impl Descriptor {
    fn validate(&self) -> Result<()> {
        let actual = descriptor(self.fd)?;
        // Socket permission bits can change when its peer disconnects on macOS;
        // the resource identity is its device, inode and file type.
        let kind = libc::S_IFMT as u32;
        if (self.dev, self.ino, self.mode & kind) != (actual.dev, actual.ino, actual.mode & kind) {
            return Err(failure(format!(
                "inherited fd identity mismatch: fd {} saved ({}, {}, {}), actual ({}, {}, {})",
                self.fd, self.dev, self.ino, self.mode, actual.dev, actual.ino, actual.mode
            )));
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
struct Snapshot {
    schema: u32,
    descriptors: Vec<Descriptor>,
    lock_ino: u64,
    lock_dev: u64,
    pen: Pen,
}
impl Snapshot {
    fn save(&self, path: &Path) -> Result<()> {
        // Recovery updates must retain the original identity baseline, even
        // when the current image's descriptors or schema failed validation.
        state::write_durable(path, &serde_json::to_value(self)?)
    }
    fn remove_backup(&mut self) {
        let fds = [
            self.pen.upgrade.control.take().map(|f| f.as_raw_fd()),
            self.pen.upgrade.alive.take().map(|f| f.as_raw_fd()),
        ];
        self.descriptors.retain(|d| !fds.contains(&Some(d.fd)));
        self.pen.upgrade.backup_pid = None;
    }
    fn validate_fd(&self, fd: i32) -> Result<()> {
        self.descriptors
            .iter()
            .find(|d| d.fd == fd)
            .ok_or_else(|| failure("missing inherited fd identity"))?
            .validate()
    }
}

fn descriptor(fd: i32) -> Result<Descriptor> {
    let mut st = unsafe { std::mem::zeroed::<libc::stat>() };
    if unsafe { libc::fstat(fd, &mut st) } < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(Descriptor {
        fd,
        dev: st.st_dev as u64,
        ino: st.st_ino,
        mode: st.st_mode as u32,
    })
}
fn flags(fd: i32, inheritable: bool) -> Result<i32> {
    let old = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if old < 0
        || unsafe {
            libc::fcntl(
                fd,
                libc::F_SETFD,
                if inheritable {
                    old & !libc::FD_CLOEXEC
                } else {
                    old | libc::FD_CLOEXEC
                },
            )
        } < 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(old)
}
fn pipe() -> Result<(Handle<File>, Handle<File>)> {
    let mut fds = [-1; 2];
    if unsafe { libc::pipe(fds.as_mut_ptr()) } < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let (r, w) = unsafe {
        (
            Handle::new(File::from_raw_fd(fds[0])),
            Handle::new(File::from_raw_fd(fds[1])),
        )
    };
    flags(fds[0], false)?;
    flags(fds[1], false)?;
    Ok((r, w))
}
fn failure(e: impl ToString) -> Error {
    Error::new(1, "upgrade_failed", e)
}

pub(crate) fn probe_target(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(failure("target must be an absolute executable path"));
    }
    let path = fs::canonicalize(path)?;
    let meta = fs::metadata(&path)?;
    if !meta.is_file() || meta.permissions().mode() & 0o111 == 0 {
        return Err(failure("target is not executable"));
    }
    let mut child = Command::new(&path)
        .arg("__pen-probe")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let result = read_ready(child.stdout.take().unwrap(), Duration::from_secs(3));
    let deadline = Instant::now() + Duration::from_secs(1);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let reply = result?;
    if reply["schema"] != SCHEMA || !status.is_some_and(|s| s.success()) {
        return Err(failure("target does not support this snapshot schema"));
    }
    Ok(path)
}

impl Pen {
    fn fds(&self) -> Vec<i32> {
        let mut fds = vec![
            self.master.as_raw_fd(),
            self.listener.as_raw_fd(),
            self.lock.as_raw_fd(),
        ];
        fds.extend(self.clients.values().map(|c| c.sock.as_raw_fd()));
        fds.extend(self.upgrade.control.iter().map(|f| f.as_raw_fd()));
        fds.extend(self.upgrade.alive.iter().map(|f| f.as_raw_fd()));
        fds
    }
    fn take_resources(&mut self) {
        self.owns = true;
        self.master.owned = true;
        self.listener.owned = true;
        self.lock.owned = true;
        for c in self.clients.values_mut() {
            c.sock.owned = true;
        }
        for f in [&mut self.upgrade.control, &mut self.upgrade.alive]
            .into_iter()
            .flatten()
        {
            f.owned = true;
        }
    }
    fn save(&self, path: &Path) -> Result<()> {
        // Only the original live pen captures resource identities. Recovery
        // persists the loaded Snapshot instead of capturing them again.
        let descriptors: Vec<_> = self
            .fds()
            .into_iter()
            .map(descriptor)
            .collect::<Result<_>>()?;
        let m = self.lock.metadata()?;
        state::write_durable(
            path,
            &json!({"schema":SCHEMA,"descriptors":descriptors,"lock_ino":m.ino(),"lock_dev":m.dev(),"pen":self}),
        )
    }
    pub(super) fn observe_exit(&mut self) -> Result<()> {
        if self.exit.is_some() {
            return Ok(());
        }
        if self.observer {
            // The standby is not the agent's parent. EOF is definitive even if a
            // dead agent remains a zombie owned by the operating system.
            if !self.pty_open
                || unsafe { libc::kill(self.agent as i32, 0) } < 0
                    && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
            {
                self.exit = Some(0);
                self.exit_at = now();
            }
        } else {
            let mut status = 0;
            let rc = unsafe { libc::waitpid(self.agent as i32, &mut status, libc::WNOHANG) };
            if rc > 0 {
                self.exit = Some(status);
                self.exit_at = now();
            }
            if rc < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        Ok(())
    }
    pub(super) fn prepare_upgrade(&mut self, req: &Value) -> Value {
        if self.custody != Custody::Owner
            || !self.upgrade.state.is_empty()
            || self.upgrade.backup_pid.is_some()
            || ["upgrade.json", "upgrade.committed", "upgrade.active"]
                .iter()
                .any(|name| self.dir.join(name).exists())
        {
            return json!({"ok":false,"error":"upgrade_busy","upgrade":self.upgrade.public()});
        }
        let path = Path::new(req["exe"].as_str().unwrap_or(""));
        let target = match probe_target(path) {
            Ok(p) => p,
            Err(e) => return e.value,
        };
        if crate::executable().ok().as_ref() == Some(&target) {
            return json!({"ok":true,"result":"already_current","exe":target,"upgrade":self.upgrade.public()});
        }
        self.upgrade = Upgrade {
            epoch: Some(
                req["epoch"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            ),
            attempt: 1,
            target: Some(target.clone()),
            old: crate::executable().ok(),
            state: "accepted".into(),
            result: Some("accepted".into()),
            ..Upgrade::default()
        };
        self.pending_upgrade = Some(target);
        json!({"ok":true,"result":"accepted","upgrade":self.upgrade.public()})
    }
    pub(super) fn handoff(&mut self, target: PathBuf) {
        let result = (|| -> Result<()> {
            let (control_read, control_write) = pipe()?;
            let (alive_read, alive_write) = pipe()?;
            self.upgrade.control = Some(control_write);
            self.upgrade.alive = Some(alive_read);
            self.custody = Custody::Handoff;
            self.save(&self.dir.join("upgrade.json"))?;
            let pid = unsafe { libc::fork() };
            if pid < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            if pid == 0 {
                // No Child/PTY destructors, inherited control writers, or normal
                // pen cleanup may run in this process.
                self.upgrade.control.take();
                self.upgrade.alive.take();
                standby(self, control_read, alive_write);
            }
            drop(control_read);
            drop(alive_write);
            self.upgrade.backup_pid = Some(pid);
            self.save(&self.dir.join("upgrade.json"))?;
            let error = exec_pen(&target, &self.dir, false, false, &self.fds());
            Err(error)
        })();
        if let Err(e) = result {
            // exec returned: the original in-memory stream state is authoritative.
            if let Some(mut control) = self.upgrade.control.take() {
                let _ = control.write_all(b"cancel");
            }
            for fd in self.fds() {
                let _ = flags(fd, false);
            }
            self.custody = if self.observer {
                Custody::Observer
            } else {
                Custody::Owner
            };
            self.upgrade.state = "cancelling".into();
            self.upgrade.result = Some("failed".into());
            self.upgrade.last_error = Some(e.value.to_string());
            self.finish_upgrade();
        }
    }
    pub(super) fn finish_upgrade(&mut self) {
        if !matches!(
            self.upgrade.state.as_str(),
            "running_new_pending" | "cancelling"
        ) {
            return;
        }
        if let Some(ref mut control) = self.upgrade.control {
            let _ = control.write_all(b"active");
        }
        self.upgrade.control.take();
        if let Some(ref alive) = self.upgrade.alive {
            let mut p = libc::pollfd {
                fd: alive.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            if unsafe { libc::poll(&mut p, 1, 0) } <= 0 {
                return;
            }
            let mut byte = [0u8];
            if unsafe { libc::read(p.fd, byte.as_mut_ptr().cast(), 1) } != 0 {
                return;
            }
        }
        if let Some(pid) = self.upgrade.backup_pid {
            let rc = unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) };
            if rc == 0 {
                return;
            }
            if rc < 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::ECHILD) {
                return;
            }
        }
        self.upgrade.alive.take();
        self.upgrade.backup_pid = None;
        for name in ["upgrade.json", "upgrade.committed", "upgrade.active"] {
            if let Err(e) = fs::remove_file(self.dir.join(name))
                && e.kind() != std::io::ErrorKind::NotFound
            {
                self.upgrade.last_error = Some(e.to_string());
                return;
            }
        }
        if self.upgrade.state == "running_new_pending" {
            self.upgrade.result = Some("complete".into());
        }
        self.upgrade.state.clear();
        self.custody = if self.observer {
            Custody::Observer
        } else {
            Custody::Owner
        };
    }
}

fn exec_pen(target: &Path, dir: &Path, observer: bool, fallback: bool, fds: &[i32]) -> Error {
    let mut changed = Vec::new();
    for &fd in fds {
        match flags(fd, true) {
            Ok(old) => changed.push((fd, old)),
            Err(e) => {
                for (fd, old) in changed {
                    unsafe {
                        libc::fcntl(fd, libc::F_SETFD, old);
                    }
                }
                return e;
            }
        }
    }
    let error = Command::new(target)
        .arg("__pen-resume")
        .arg(dir)
        .arg(if observer { "observer" } else { "parent" })
        .arg(if fallback { "fallback" } else { "target" })
        .exec();
    for (fd, old) in changed {
        unsafe {
            libc::fcntl(fd, libc::F_SETFD, old);
        }
    }
    error.into()
}

fn standby(pen: &mut Pen, mut control: Handle<File>, alive: Handle<File>) -> ! {
    let mut byte = [0u8];
    loop {
        match control.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => unsafe { libc::_exit(0) },
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => unsafe { libc::_exit(1) },
        }
    }
    if pen.dir.join("upgrade.active").exists() {
        unsafe { libc::_exit(1) }
    }
    let path = snapshot_path(&pen.dir);
    let result = (|| -> Result<()> {
        let mut snap: Snapshot = serde_json::from_slice(&fs::read(&path)?)?;
        validate(&snap, false)?;
        // K owns different pipe ends. Its failed predecessor must not be kept
        // artificially alive by any inherited writer.
        snap.remove_backup();
        snap.pen.observer = true;
        snap.pen.custody = Custody::Handoff;
        snap.save(&path)?;
        drop(control);
        drop(alive);
        let fds = snap.pen.fds();
        for target in [
            snap.pen.upgrade.old.as_ref(),
            snap.pen.upgrade.target.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            let _ = exec_pen(target, &pen.dir, true, false, &fds);
        }
        snap.pen.upgrade.state = "hold_unprotected".into();
        snap.pen.upgrade.last_error =
            Some("standby could not execute either recovery image".into());
        hold(snap, path);
    })();
    let _ = result;
    unsafe { libc::_exit(1) }
}

fn snapshot_path(dir: &Path) -> PathBuf {
    let committed = dir.join("upgrade.committed");
    if committed.exists() {
        committed
    } else {
        dir.join("upgrade.json")
    }
}
fn validate_roles(snap: &Snapshot) -> Result<()> {
    if snap.schema != SCHEMA {
        return Err(failure("incompatible snapshot schema"));
    }
    let p = &snap.pen;
    if p.dir.join("upgrade.active").exists() {
        return Err(failure("snapshot already crossed the active I/O boundary"));
    }
    let expected: BTreeSet<_> = p.fds().into_iter().collect();
    if expected.len() != p.fds().len()
        || expected.len() != snap.descriptors.len()
        || expected != snap.descriptors.iter().map(|d| d.fd).collect()
    {
        return Err(failure("invalid fd role table"));
    }
    if p.clients.iter().any(|(fd, c)| *fd != c.sock.as_raw_fd()) {
        return Err(failure("client fd role mismatch"));
    }
    Ok(())
}
fn validate(snap: &Snapshot, pipes: bool) -> Result<()> {
    validate_roles(snap)?;
    let p = &snap.pen;
    for d in &snap.descriptors {
        if !pipes
            && [
                p.upgrade.control.as_ref().map(|f| f.as_raw_fd()),
                p.upgrade.alive.as_ref().map(|f| f.as_raw_fd()),
            ]
            .contains(&Some(d.fd))
        {
            continue;
        }
        d.validate()?;
    }
    let lock = fs::metadata(p.dir.join("lock"))?;
    let held = p.lock.metadata()?;
    if (lock.dev(), lock.ino()) != (snap.lock_dev, snap.lock_ino)
        || (held.dev(), held.ino()) != (lock.dev(), lock.ino())
    {
        return Err(failure("name lock inode changed"));
    }
    if unsafe { libc::flock(p.lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } < 0 {
        return Err(failure("name lock no longer held"));
    }
    Ok(())
}

pub(crate) fn resume(args: &[String]) -> i32 {
    let Some(dir) = args.first().map(PathBuf::from) else {
        return 1;
    };
    let path = snapshot_path(&dir);
    // Loading and validation perform no I/O on any stream and do not acquire
    // automatic descriptor ownership, including on a partially decoded snapshot.
    let mut snap: Snapshot = match fs::read(&path)
        .map_err(Error::from)
        .and_then(|b| Ok(serde_json::from_slice(&b)?))
    {
        Ok(s) => s,
        Err(_) => return 1,
    };
    let validation = validate(&snap, true);
    let pen = &mut snap.pen;
    pen.observer |= args.get(1).is_some_and(|s| s == "observer");
    let result = validation.and_then(|()| {
        if path != dir.join("upgrade.committed") {
            fs::rename(&path, dir.join("upgrade.committed"))?;
        }
        if state::read(dir.join("meta.json"))["instance"] != pen.instance {
            return Err(failure("metadata instance mismatch"));
        }
        let mut meta = pen.metadata.clone();
        meta["pen_pid"] = json!(std::process::id());
        meta["exe"] = json!(crate::executable()?);
        meta["version"] = json!(env!("CARGO_PKG_VERSION"));
        state::write(&dir.join("meta.json"), &meta)?;
        state::write(&dir.join("labels.json"), &pen.labels)?;
        pen.metadata = meta;
        for fd in pen.fds() {
            flags(fd, false)?;
        }
        fs::rename(dir.join("upgrade.committed"), dir.join("upgrade.active"))?;
        Ok(())
    });
    if let Err(e) = result {
        pen.upgrade.last_error = Some(e.value.to_string());
        let path = snapshot_path(&dir);
        // Preserve the exact stream snapshot; only upgrade metadata changes.
        let _ = snap.save(&path);
        let pen = &snap.pen;
        if args.get(2).is_none_or(|s| s != "fallback")
            && let Some(old) = &pen.upgrade.old
        {
            let _ = exec_pen(old, &dir, pen.observer, true, &pen.fds());
        }
        hold(snap, path);
    }
    let mut pen = snap.pen;
    pen.custody = Custody::Handoff;
    pen.upgrade.state = "running_new_pending".into();
    // A fallback image restores service, but cannot complete the requested target.
    if crate::executable().ok().as_ref() != pen.upgrade.target.as_ref() {
        pen.upgrade.state = "cancelling".into();
        pen.upgrade.result = Some("failed".into());
        if pen.upgrade.last_error.is_none() {
            pen.upgrade.last_error =
                Some("target did not reach the active boundary; restored previous image".into());
        }
    }
    pen.take_resources();
    match pen.run() {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

fn hold(mut snap: Snapshot, path: PathBuf) -> ! {
    let roles_valid = validate_roles(&snap).is_ok();
    let listener_valid = roles_valid && snap.validate_fd(snap.pen.listener.as_raw_fd()).is_ok();
    // Even Hold's control I/O and pipe cleanup require the saved identities.
    // A rejected master/client never gains ownership by entering Hold.
    let valid_pipes: Vec<_> = [&snap.pen.upgrade.control, &snap.pen.upgrade.alive]
        .into_iter()
        .flatten()
        .filter(|f| roles_valid && snap.validate_fd(f.as_raw_fd()).is_ok())
        .map(|f| f.as_raw_fd())
        .collect();
    for fd in snap.pen.fds() {
        let _ = flags(fd, false);
    }
    for pipe in [&mut snap.pen.upgrade.control, &mut snap.pen.upgrade.alive]
        .into_iter()
        .flatten()
    {
        pipe.owned = valid_pipes.contains(&pipe.as_raw_fd());
    }
    // Never poll the original master or clients here. Only fresh control
    // connections are read, answered and closed; they are absent from snapshots.
    loop {
        if let Some(alive) = snap.pen.upgrade.alive.as_ref().filter(|p| p.owned) {
            let mut p = libc::pollfd {
                fd: alive.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            if unsafe { libc::poll(&mut p, 1, 0) } > 0 {
                let mut b = [0u8];
                if unsafe { libc::read(p.fd, b.as_mut_ptr().cast(), 1) } == 0 {
                    if let Some(pid) = snap.pen.upgrade.backup_pid {
                        unsafe {
                            libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG);
                        }
                    }
                    snap.remove_backup();
                }
            }
        }
        let pen = &mut snap.pen;
        pen.upgrade.state = if pen.upgrade.alive.is_some() {
            "hold"
        } else {
            "hold_unprotected"
        }
        .into();
        if !listener_valid {
            std::thread::sleep(Duration::from_millis(25));
            continue;
        }
        match pen.listener.accept() {
            Ok((mut sock, _)) => {
                let _ = sock.set_read_timeout(Some(Duration::from_millis(250)));
                let _ = sock.set_write_timeout(Some(Duration::from_millis(250)));
                let request = sock
                    .try_clone()
                    .map_err(Error::from)
                    .and_then(|s| read_ready(s, Duration::from_millis(250)));
                let mut target = None;
                let mut reply = match request {
                    Ok(req) if req["op"] == "status" => pen.status(),
                    Ok(req)
                        if req["op"] == "recover"
                            && (req["instance"].is_null() || req["instance"] == pen.instance)
                            && (req["epoch"].is_null()
                                || req["epoch"] == json!(pen.upgrade.epoch))
                            && (req["attempt"].is_null()
                                || req["attempt"] == pen.upgrade.attempt + 1) =>
                    {
                        match probe_target(Path::new(req["exe"].as_str().unwrap_or(""))) {
                            Ok(exe) => {
                                pen.upgrade.attempt += 1;
                                pen.upgrade.target = Some(exe.clone());
                                pen.upgrade.result = Some("accepted".into());
                                pen.upgrade.last_error = None;
                                match snap.save(&path) {
                                    Ok(()) => {
                                        target = Some(exe);
                                        json!({"ok":true,"result":"accepted","upgrade":snap.pen.upgrade.public()})
                                    }
                                    Err(e) => {
                                        snap.pen.upgrade.result = Some("failed".into());
                                        snap.pen.upgrade.last_error = Some(e.value.to_string());
                                        e.value
                                    }
                                }
                            }
                            Err(e) => e.value,
                        }
                    }
                    _ => {
                        json!({"ok":false,"error":"hold","message":"only status and recover are available in Hold"})
                    }
                };
                reply["proto"] = json!(1);
                let _ = writeln!(sock, "{reply}");
                drop(sock);
                if let Some(target) = target {
                    let pen = &mut snap.pen;
                    let e = exec_pen(&target, &pen.dir, pen.observer, false, &pen.fds());
                    pen.upgrade.result = Some("failed".into());
                    pen.upgrade.last_error = Some(e.value.to_string());
                    let _ = snap.save(&path);
                }
            }
            Err(_) => std::thread::sleep(Duration::from_millis(25)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_identity_survives_peer_disconnect() {
        let (socket, peer) = UnixStream::pair().unwrap();
        let saved = descriptor(socket.as_raw_fd()).unwrap();
        drop(peer);
        saved.validate().unwrap();
        let (replacement, _peer) = UnixStream::pair().unwrap();
        assert_eq!(
            unsafe { libc::dup2(replacement.as_raw_fd(), socket.as_raw_fd()) },
            socket.as_raw_fd()
        );
        assert!(saved.validate().is_err(), "a different socket was accepted");
    }
}
