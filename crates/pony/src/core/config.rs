use super::{Error, Result, files};
use crate::chats::CHATS;
use serde::Deserialize;
use std::{
    collections::HashSet,
    fs, io,
    io::Write,
    path::{Path, PathBuf},
};
use toml_edit::{DocumentMut, InlineTable, Item, Table, Value, value};

#[cfg(test)]
mod tests;

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
            if !name_segment(&repo) {
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
    Ok(load_with_source(home)?.map(|(config, _)| config))
}

/// Keep exactly the bytes that were validated and adopted by the runtime.
pub fn load_with_source(home: &Path) -> Result<Option<(Config, ConfigFile)>> {
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
    let config = parse(&source, &path)?;
    Ok(Some((config, ConfigFile { path, source })))
}

fn parse(source: &str, path: &Path) -> Result<Config> {
    let syntax_error = |error: toml::de::Error| {
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
    };
    let document = toml::de::DeTable::parse(source).map_err(syntax_error)?;
    validate_keys(document.get_ref(), source, path)?;
    let table =
        toml::Table::deserialize(toml::de::Deserializer::from(document)).map_err(syntax_error)?;
    let problem = |p: Problem| {
        Error::new(
            "config_invalid",
            format!("{}:{}: {}", path.display(), p.position, p.reason),
        )
    };
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
    Ok(Config {
        corral,
        proxy,
        chats,
    })
}

pub(crate) fn name_segment(s: &str) -> bool {
    s.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

pub struct ConfigFile {
    path: PathBuf,
    source: String,
}

impl ConfigFile {
    pub fn bind(&self, kind: &str, channel: &str, repo: &str) -> Result<()> {
        self.prepare_binding(kind, channel, repo)?.commit()
    }

    fn current(&self) -> Result<String> {
        fs::read_to_string(&self.path).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                Error::new("changed", "configuration changed")
            } else if error.kind() == io::ErrorKind::InvalidData {
                Error::new("config_invalid", "configuration is not UTF-8")
            } else {
                files::io_error(&self.path, error)
            }
        })
    }

    fn unchanged(&self, current: &str) -> Result<()> {
        if current != self.source {
            return Err(Error::new("changed", "configuration changed"));
        }
        Ok(())
    }

    fn prepare_binding(&self, kind: &str, channel: &str, repo: &str) -> Result<Binding<'_>> {
        let current = self.current()?;
        parse(&current, &self.path)?;
        self.unchanged(&current)?;
        let mut document: DocumentMut = current
            .parse()
            .map_err(|_| Error::new("config_invalid", "invalid TOML"))?;
        if document.to_string() != current {
            return Err(Error::new(
                "format_not_preserved",
                "cannot preserve TOML formatting",
            ));
        }
        let channels = document
            .get_mut(kind)
            .and_then(Item::as_table_like_mut)
            .and_then(|table| table.get_mut("channels"))
            .ok_or_else(|| Error::new("config_invalid", "missing channel bindings"))?;
        if let Some(array) = channels.as_array_mut() {
            let mut binding = InlineTable::new();
            binding.insert("id", Value::from(channel));
            binding.insert("repo", Value::from(repo));
            array.push(binding);
        } else if let Some(array) = channels.as_array_of_tables_mut() {
            let mut binding = Table::new();
            binding.insert("id", value(channel));
            binding.insert("repo", value(repo));
            array.push(binding);
        } else {
            return Err(Error::new("config_invalid", "expected channel bindings"));
        }
        let candidate = document.to_string();
        parse(&candidate, &self.path)?;
        let prepare = || -> io::Result<tempfile::NamedTempFile> {
            let mut temporary = tempfile::NamedTempFile::new_in(self.path.parent().unwrap())?;
            temporary.write_all(candidate.as_bytes())?;
            temporary
                .as_file()
                .set_permissions(fs::metadata(&self.path)?.permissions())?;
            temporary.as_file().sync_all()?;
            Ok(temporary)
        };
        let temporary = prepare().map_err(|error| files::io_error(&self.path, error))?;
        Ok(Binding {
            config: self,
            temporary,
        })
    }
}

struct Binding<'a> {
    config: &'a ConfigFile,
    temporary: tempfile::NamedTempFile,
}

impl Binding<'_> {
    fn commit(self) -> Result<()> {
        self.config.unchanged(&self.config.current()?)?;
        self.temporary
            .persist(&self.config.path)
            .map_err(|error| files::io_error(&self.config.path, error.error))?;
        Ok(())
    }
}

fn validate_keys(table: &toml::de::DeTable<'_>, source: &str, path: &Path) -> Result<()> {
    reject_unknown_keys(
        table,
        |key| key == "corral" || key == "proxy" || CHATS.iter().any(|chat| chat.kind() == key),
        "",
        source,
        path,
    )?;
    for chat in CHATS {
        let Some(table) = table
            .get(chat.kind())
            .and_then(|value| value.get_ref().as_table())
        else {
            continue;
        };
        reject_unknown_keys(
            table,
            |key| key == "owner" || key == "channels" || chat.config_keys().contains(&key),
            &format!("{}.", chat.kind()),
            source,
            path,
        )?;
        if let Some(channels) = table
            .get("channels")
            .and_then(|value| value.get_ref().as_array())
        {
            for (i, channel) in channels.iter().enumerate() {
                if let Some(table) = channel.get_ref().as_table() {
                    reject_unknown_keys(
                        table,
                        |key| key == "id" || key == "repo",
                        &format!("{}.channels[{i}].", chat.kind()),
                        source,
                        path,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn reject_unknown_keys(
    table: &toml::de::DeTable<'_>,
    allowed: impl Fn(&str) -> bool,
    prefix: &str,
    source: &str,
    path: &Path,
) -> Result<()> {
    for key in table.keys() {
        if allowed(key.get_ref()) {
            continue;
        }
        let span = key.span();
        let line = source.as_bytes()[..span.start]
            .iter()
            .filter(|&&b| b == b'\n')
            .count()
            + 1;
        let raw = &source[span];
        // Quoted keys can contain arbitrary user data; report only their line.
        let position = if !raw.is_empty()
            && raw
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            format!(" {prefix}{raw}")
        } else {
            String::new()
        };
        return Err(Error::new(
            "config_invalid",
            format!("{}:{line}: unknown key{position}", path.display()),
        ));
    }
    Ok(())
}
