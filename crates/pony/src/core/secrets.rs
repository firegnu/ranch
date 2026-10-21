use super::{Error, Result, files};
use crate::chats::Chat;
use std::{
    collections::BTreeMap,
    ffi::CString,
    fs::File,
    io::{self, Read},
    os::fd::{AsRawFd, FromRawFd},
    path::Path,
};

/// No Debug/Serialize wrapper: credential values are only passed to their adapter.
pub struct Credentials(pub BTreeMap<&'static str, String>);

fn open_at(parent: &File, name: &str, path: &Path, directory: bool) -> Result<File> {
    let name = CString::new(name).expect("registered credential path has no NUL");
    let flags = libc::O_RDONLY
        | libc::O_NOFOLLOW
        | libc::O_CLOEXEC
        | libc::O_NONBLOCK
        | if directory { libc::O_DIRECTORY } else { 0 };
    let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags) };
    if fd < 0 {
        return Err(
            if io::Error::last_os_error().kind() == io::ErrorKind::NotFound {
                Error::new(
                    "secrets_missing",
                    format!(
                        "{}: credential file or directory is missing",
                        path.display()
                    ),
                )
            } else {
                files::unsafe_path(path)
            },
        );
    }
    // The descriptor is freshly opened and now owned solely by this File.
    let file = unsafe { File::from_raw_fd(fd) };
    files::check(&file, path, directory)?;
    Ok(file)
}

pub fn load(home: &Path, chat: &dyn Chat) -> Result<Credentials> {
    let root = files::directory(home, false)?;
    let directory = open_at(&root, "secrets", &home.join("secrets"), true)?;
    let name = format!("{}.toml", chat.kind());
    let path = home.join("secrets").join(&name);
    let mut file = open_at(&directory, &name, &path, false)?;
    let invalid = |line: usize| {
        Error::new(
            "secrets_invalid",
            format!("{}:{line}: invalid credentials", path.display()),
        )
    };
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|_| invalid(1))?;
    let source =
        std::str::from_utf8(&bytes).map_err(|e| invalid(line_at(&bytes, e.valid_up_to())))?;
    let table: toml::Table = toml::from_str(source)
        .map_err(|e: toml::de::Error| invalid(line_at(&bytes, e.span().map_or(0, |s| s.start))))?;
    let mut credentials = Credentials(BTreeMap::new());
    for key in chat.secret_keys() {
        let value = table
            .get(*key)
            .and_then(toml::Value::as_str)
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| invalid(1))?;
        credentials.0.insert(key, value.into());
    }
    Ok(credentials)
}

fn line_at(source: &[u8], offset: usize) -> usize {
    source[..offset.min(source.len())]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        + 1
}
