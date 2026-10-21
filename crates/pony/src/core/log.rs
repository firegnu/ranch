use super::{Result, files, now};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub struct Log(PathBuf);

impl Log {
    pub fn new(home: &Path) -> Result<Self> {
        let directory = home.join("logs");
        files::directory(&directory, true)?;
        Ok(Self(directory))
    }

    // Only fixed event/error codes belong here, never configuration or credential contents.
    pub fn event(&self, code: &str) -> Result<()> {
        let path = self.0.join("pony.log");
        let line = format!("{:.3} {code}\n", now());
        let mut file = files::output(&path, true)?;
        let size = file
            .metadata()
            .map_err(|e| files::io_error(&path, e))?
            .len();
        if size + line.len() as u64 >= 1024 * 1024 {
            drop(file);
            for i in (1..=3).rev() {
                let from = self.0.join(if i == 1 {
                    "pony.log".into()
                } else {
                    format!("pony.log.{}", i - 1)
                });
                let to = self.0.join(format!("pony.log.{i}"));
                match fs::rename(&from, &to) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(files::io_error(&from, error)),
                }
            }
            file = files::output(&path, true)?;
        }
        file.write_all(line.as_bytes())
            .map_err(|e| files::io_error(&path, e))
    }
}
