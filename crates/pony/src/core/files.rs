use super::{Error, Result};
use std::{
    fs::{self, File, OpenOptions},
    io,
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::Path,
};

pub fn io_error(path: &Path, error: io::Error) -> Error {
    Error::new("io", format!("{}: {error}", path.display()))
}

pub fn check(file: &File, path: &Path, directory: bool) -> Result<()> {
    let metadata = file.metadata().map_err(|e| io_error(path, e))?;
    let mode = if directory { 0o700 } else { 0o600 };
    if metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o7777 != mode
        || (directory && !metadata.is_dir())
        || (!directory && !metadata.is_file())
    {
        return Err(unsafe_path(path));
    }
    Ok(())
}

pub fn unsafe_path(path: &Path) -> Error {
    Error::new(
        "secrets_too_open",
        format!(
            "{}: expected a private, current-user-owned directory (0700) or regular file (0600); symbolic links are not allowed",
            path.display()
        ),
    )
}

pub fn directory(path: &Path, create: bool) -> Result<File> {
    if create {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)
            .map_err(|e| io_error(path, e))?;
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| unsafe_path(path))?;
    check(&file, path, true)?;
    Ok(file)
}

pub fn output(path: &Path, append: bool) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .append(append)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| io_error(path, e))?;
    check(&file, path, false)?;
    Ok(file)
}
