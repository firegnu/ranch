//! Local colors used for OSC 10/11 replies while no client is attached.
use crate::{Error, Result, state};
use serde::{Deserialize, Serialize};
use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf};

#[derive(Serialize, Deserialize)]
pub(crate) struct Colors {
    pub fg: String,
    pub bg: String,
}
impl Colors {
    pub fn new(fg: &str, bg: &str) -> Result<Self> {
        for color in [fg, bg] {
            if color.len() != 7
                || !color.starts_with('#')
                || !color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
            {
                return Err(Error::new(1, "usage", "COLOR must be #rrggbb"));
            }
        }
        Ok(Self {
            fg: fg.to_ascii_lowercase(),
            bg: bg.to_ascii_lowercase(),
        })
    }
    pub fn reply(&self, osc: u8, bel: bool) -> Vec<u8> {
        let color = if osc == 10 { &self.fg } else { &self.bg };
        let (r, g, b) = (&color[1..3], &color[3..5], &color[5..7]);
        let end = if bel { "\x07" } else { "\x1b\\" };
        format!("\x1b]{osc};rgb:{r}{r}/{g}{g}/{b}{b}{end}").into_bytes()
    }
}

fn path() -> PathBuf {
    state::home().join(".runtime/terminal-colors.json")
}
pub(crate) fn read() -> Result<Option<Colors>> {
    let path = path();
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let invalid = |e: Error| {
        Error::new(
            1,
            "invalid_settings",
            format!(
                "{}: {}",
                path.display(),
                e.value["message"].as_str().unwrap_or("invalid colors")
            ),
        )
    };
    let colors: Colors = serde_json::from_slice(&bytes).map_err(|e| invalid(e.into()))?;
    Colors::new(&colors.fg, &colors.bg)
        .map(Some)
        .map_err(invalid)
}
pub(crate) fn set(colors: Option<&Colors>) -> Result<()> {
    let path = path();
    match colors {
        Some(colors) => {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(path.parent().unwrap())?;
            state::write(&path, &serde_json::to_value(colors)?)
        }
        None => match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        },
    }
}
