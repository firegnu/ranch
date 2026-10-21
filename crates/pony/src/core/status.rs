use super::{
    Error, Result,
    config::{self, Config},
    files, lock, now,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct ChannelStatus {
    id: String,
    repo: String,
    name: Option<String>,
    private: Option<bool>,
    problem: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChatStatus {
    pub kind: String,
    pub connection: String,
    pub since: Option<f64>,
    reason: Option<String>,
    message: Option<String>,
    workspace: Option<String>,
    bot: Option<String>,
    other_connections: u32,
    channels: Vec<ChannelStatus>,
}

#[derive(Serialize, Deserialize)]
pub struct Exit {
    at: f64,
    code: String,
    message: String,
}

#[derive(Serialize, Deserialize)]
pub struct Snapshot {
    pub pid: u32,
    pub started: f64,
    pub config_error: Option<Error>,
    pub last_exit: Option<Exit>,
    pub chats: Vec<ChatStatus>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self::new()
    }
}

pub fn chats(config: &Config, connection: &str, since: Option<f64>) -> Vec<ChatStatus> {
    config
        .chats
        .iter()
        .map(|chat| ChatStatus {
            kind: chat.kind.clone(),
            connection: connection.into(),
            since,
            reason: None,
            message: None,
            workspace: None,
            bot: None,
            other_connections: 0,
            channels: chat
                .channels
                .iter()
                .map(|channel| ChannelStatus {
                    id: channel.id.clone(),
                    repo: channel.repo.clone(),
                    name: None,
                    private: None,
                    problem: None,
                })
                .collect(),
        })
        .collect()
}

impl Snapshot {
    pub fn update(&mut self, engine: &super::engine::Engine, at: f64) {
        for chat in &mut self.chats {
            if let Some(report) = engine.reports.get(&chat.kind) {
                if chat.connection != report.connection {
                    chat.since = Some(at);
                }
                chat.connection = report.connection.clone();
                chat.reason = if report.connection == "failed" {
                    report.reason.clone()
                } else {
                    None
                };
                chat.message = if report.connection == "failed" {
                    report.message.clone()
                } else {
                    None
                };
                chat.workspace = report.workspace.clone();
                chat.bot = report.bot.clone();
                chat.other_connections = report.other_connections;
            }
            for channel in &mut chat.channels {
                if let Some(info) = engine
                    .channels
                    .get(&(chat.kind.clone(), channel.id.clone()))
                {
                    channel.name = info.name.clone();
                    channel.private = info.private;
                    channel.problem = info.problem.clone();
                }
            }
        }
    }
    pub fn new() -> Self {
        Self {
            pid: std::process::id(),
            started: now(),
            config_error: None,
            last_exit: None,
            chats: Vec::new(),
        }
    }

    pub fn exit(&mut self, code: &str, message: &str) {
        let at = now();
        self.last_exit = Some(Exit {
            at,
            code: code.into(),
            message: message.into(),
        });
        for chat in &mut self.chats {
            chat.connection = "off".into();
            chat.since = Some(at);
        }
    }

    pub fn save(&self, home: &Path) -> Result<()> {
        let directory = home.join("run");
        let temporary = directory.join(format!(".status.{}.{}.tmp", self.pid, now().to_bits()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|e| files::io_error(&temporary, e))?;
        let result = (|| {
            let bytes =
                serde_json::to_vec(self).expect("status contains only JSON-compatible fields");
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temporary, directory.join("status.json"))
        })()
        .map_err(|e| files::io_error(&directory.join("status.json"), e));
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

pub fn read(home: &Path) -> Value {
    let snapshot: Option<Snapshot> = fs::read(home.join("run/status.json"))
        .ok()
        .and_then(|s| serde_json::from_slice(&s).ok());
    let owner = lock::holder(home);
    let live = snapshot.as_ref().filter(|s| {
        owner
            .as_ref()
            .is_some_and(|o| o.pid == s.pid && o.started == s.started)
    });
    let (mut config, mut config_message, mut chats) = match config::load(home) {
        Ok(None) => (
            "missing",
            Some(format!(
                "{}: file is missing",
                home.join("config.toml").display()
            )),
            Vec::new(),
        ),
        Err(error) => ("invalid", Some(error.message), Vec::new()),
        Ok(Some(config)) => (
            "ok",
            None,
            chats(
                &config,
                "off",
                snapshot
                    .as_ref()
                    .and_then(|s| s.last_exit.as_ref())
                    .map(|e| e.at),
            ),
        ),
    };
    if config != "missing" && owner.is_some() {
        if let Some(snapshot) = live {
            chats = snapshot.chats.clone();
        }
        for chat in &mut chats {
            if chat.connection == "off" {
                chat.connection = "connecting".into();
                chat.since = None;
            }
        }
    }
    if config == "ok" {
        let recorded = if owner.is_some() {
            live
        } else {
            snapshot.as_ref()
        };
        if let Some(error) = recorded
            .and_then(|s| s.config_error.as_ref())
            .filter(|e| e.code.starts_with("secrets_"))
        {
            config = "invalid";
            config_message = Some(error.message.clone());
        }
    }
    json!({
        "ok": true, "contract": "1", "version": env!("CARGO_PKG_VERSION"),
        "config": config, "config_message": config_message,
        "service": if cfg!(target_os = "macos") { "not_installed" } else { "unsupported" },
        "running": owner.is_some(), "pid": owner.as_ref().map(|o| o.pid), "started": owner.as_ref().map(|o| o.started),
        "last_exit": if owner.is_some() { None } else { snapshot.as_ref().and_then(|s| s.last_exit.as_ref()) }, "chats": chats,
    })
}
