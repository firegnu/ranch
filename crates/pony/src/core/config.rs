use super::{Error, Result, files};
use crate::chats::CHATS;
use std::{collections::HashSet, fs, io, path::Path};

#[derive(Clone, PartialEq)]
pub struct Config {
    pub corral: Option<String>,
    pub proxy: Option<String>,
    pub chats: Vec<ChatConfig>,
}

#[derive(Clone, PartialEq)]
pub struct ChatConfig {
    pub kind: String,
    pub owner: String,
    pub channels: Vec<Channel>,
}

#[derive(Clone, PartialEq)]
pub struct Channel {
    pub id: String,
    pub repo: String,
}

pub struct Problem {
    pub position: String,
    pub reason: &'static str,
}

impl Problem {
    pub fn new(position: impl Into<String>, reason: &'static str) -> Self {
        Self {
            position: position.into(),
            reason,
        }
    }
}

impl ChatConfig {
    pub fn parse(kind: &str, table: &toml::Table) -> std::result::Result<Self, Problem> {
        let owner = string(table.get("owner"), "owner")?;
        let channels = table
            .get("channels")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| Problem::new("channels", "expected an array of channel bindings"))?;
        let mut parsed = Vec::new();
        let mut ids = HashSet::new();
        let mut repos = HashSet::new();
        for (i, channel) in channels.iter().enumerate() {
            let position = format!("channels[{i}]");
            let table = channel
                .as_table()
                .ok_or_else(|| Problem::new(&position, "expected a channel binding"))?;
            let id = string(table.get("id"), &format!("{position}.id"))?;
            let repo = string(table.get("repo"), &format!("{position}.repo"))?;
            if !repo.as_bytes()[0].is_ascii_alphanumeric()
                || !repo
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            {
                return Err(Problem::new(
                    format!("{position}.repo"),
                    "expected an ASCII repository name starting with a letter or digit and containing only letters, digits, '.', '_' or '-'",
                ));
            }
            if !ids.insert(id.clone()) {
                return Err(Problem::new(
                    format!("{position}.id"),
                    "channel is bound more than once",
                ));
            }
            if !repos.insert(repo.clone()) {
                return Err(Problem::new(
                    format!("{position}.repo"),
                    "repository is bound more than once",
                ));
            }
            parsed.push(Channel { id, repo });
        }
        Ok(Self {
            kind: kind.into(),
            owner,
            channels: parsed,
        })
    }
}

fn string(value: Option<&toml::Value>, position: &str) -> std::result::Result<String, Problem> {
    value
        .and_then(toml::Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| Problem::new(position, "expected a nonempty string"))
}

pub fn load(home: &Path) -> Result<Option<Config>> {
    let path = home.join("config.toml");
    if let Err(error) = fs::symlink_metadata(&path) {
        if error.kind() == io::ErrorKind::NotFound {
            return Ok(None);
        }
        return Err(Error::new(
            "config_invalid",
            format!("{}: file: {error}", path.display()),
        ));
    }
    files::directory(home, false)?;
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(Error::new(
                "config_invalid",
                format!("{}: file: {error}", path.display()),
            ));
        }
    };
    let table: toml::Table = toml::from_str(&source).map_err(|error: toml::de::Error| {
        let offset = error.span().map_or(0, |span| span.start).min(source.len());
        let prefix = &source.as_bytes()[..offset];
        let line = prefix.iter().filter(|&&b| b == b'\n').count() + 1;
        let column = prefix
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(offset + 1, |p| offset - p);
        Error::new(
            "config_invalid",
            format!("{}:{line}:{column}: invalid TOML", path.display()),
        )
    })?;
    let problem = |p: Problem| {
        Error::new(
            "config_invalid",
            format!("{}:{}: {}", path.display(), p.position, p.reason),
        )
    };
    for key in table.keys() {
        if key != "corral" && key != "proxy" && !CHATS.iter().any(|chat| chat.kind() == key) {
            return Err(problem(Problem::new("root", "unknown top-level key")));
        }
    }
    let corral = table
        .get("corral")
        .map(|v| string(Some(v), "corral"))
        .transpose()
        .map_err(problem)?;
    if corral.as_ref().is_some_and(|p| !Path::new(p).is_absolute()) {
        return Err(problem(Problem::new(
            "corral",
            "expected an absolute executable path",
        )));
    }
    let proxy = table
        .get("proxy")
        .map(|v| string(Some(v), "proxy"))
        .transpose()
        .map_err(problem)?;
    let mut chats = Vec::new();
    for chat in CHATS {
        if let Some(value) = table.get(chat.kind()) {
            let table = value
                .as_table()
                .ok_or_else(|| problem(Problem::new(chat.kind(), "expected an adapter table")))?;
            chats.push(chat.read_config(table).map_err(|p| {
                problem(Problem::new(
                    format!("{}.{}", chat.kind(), p.position),
                    p.reason,
                ))
            })?);
        }
    }
    Ok(Some(Config {
        corral,
        proxy,
        chats,
    }))
}
