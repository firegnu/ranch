use super::{Error, Result, files};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    path::Path,
};

#[derive(Serialize, Deserialize)]
pub struct Owner {
    pub pid: u32,
    pub started: f64,
}

// Never unlink the lock or reopen it in the holder: closing any descriptor for
// the inode releases this process's POSIX record lock.
pub struct Lock {
    _file: File,
}

fn request() -> libc::flock {
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = libc::F_WRLCK as libc::c_short;
    lock.l_whence = libc::SEEK_SET as libc::c_short;
    lock
}

impl Lock {
    pub fn acquire(home: &Path, started: f64) -> Result<Self> {
        let path = home.join("run/lock");
        let mut file = files::output(&path, false)?;
        if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLK, &request()) } < 0 {
            let error = io::Error::last_os_error();
            return Err(
                if matches!(
                    error.raw_os_error(),
                    Some(libc::EACCES) | Some(libc::EAGAIN)
                ) {
                    Error::new(
                        "already_running",
                        "another pony run holds this PONY_HOME lock",
                    )
                } else {
                    files::io_error(&path, error)
                },
            );
        }
        let owner = Owner {
            pid: std::process::id(),
            started,
        };
        let bytes = serde_json::to_vec(&owner).expect("lock metadata is JSON-compatible");
        file.set_len(0)
            .and_then(|()| file.write_all(&bytes))
            .map_err(|e| files::io_error(&path, e))?;
        Ok(Self { _file: file })
    }
}

pub fn holder(home: &Path) -> Option<Owner> {
    let path = home.join("run/lock");
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(&path)
        .ok()?;
    files::check(&file, &path, false).ok()?;
    let mut lock = request();
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETLK, &mut lock) } < 0
        || lock.l_type == libc::F_UNLCK as libc::c_short
    {
        return None;
    }
    let pid = u32::try_from(lock.l_pid).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    let metadata: Option<Owner> = serde_json::from_slice(&bytes).ok();
    Some(Owner {
        pid,
        started: metadata.filter(|m| m.pid == pid).map_or(0.0, |m| m.started),
    })
}
