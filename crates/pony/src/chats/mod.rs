//! Transport boundary. Adapters own network workers, retries and platform syntax.
mod slack;

use crate::core::config::{ChatConfig, Problem};
use crate::core::{Result, secrets::Credentials};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub channel: String,
    pub id: String,
    pub thread: Option<String>,
    pub sender: String,
    pub mentioned: bool,
    pub text: String,
    pub attachments: bool,
    pub at: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelInfo {
    pub id: String,
    pub name: Option<String>,
    pub private: Option<bool>,
    /// None means the bot is a member. Otherwise: not_member, not_found, archived.
    pub problem: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub connection: String,
    /// Public, sanitized error code and explanation; never include network URLs or tokens.
    pub reason: Option<String>,
    pub message: Option<String>,
    pub workspace: Option<String>,
    pub bot: Option<String>,
    pub other_connections: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Event {
    Message(Message),
    Deleted { channel: String, id: String },
    Connection(ConnectionInfo),
    Ignored,
}

/// Calling emit durably records messages before returning. ACK only after Ok.
/// A worker can emit while the core is waiting for corral or outgoing delivery.
#[derive(Clone)]
pub struct Events(pub Arc<dyn Fn(Event) -> Result<()> + Send + Sync>);
impl Events {
    pub fn emit(&self, event: Event) -> Result<()> {
        (self.0)(event)
    }
}

pub struct Connect {
    pub config: ChatConfig,
    pub proxy: Option<String>,
    /// Call before EVERY connection/reconnection; permissions and values are reread.
    pub credentials: Arc<dyn Fn() -> Result<Credentials> + Send + Sync>,
    pub events: Events,
}

#[derive(Clone, Copy)]
pub struct Limits {
    pub characters: usize,
    pub threads: bool,
}

pub struct Outgoing<'a> {
    pub channel: &'a str,
    pub thread: Option<&'a str>,
    pub markdown: &'a str,
    pub notify: bool,
}

#[derive(Debug)]
pub struct SendError {
    pub code: String,
    pub resize: bool,
}

pub trait Connection {
    fn name(&self) -> &str {
        "聊天"
    }
    fn limits(&self) -> Limits;
    /// Return the posted message ID. Rate limits/retries/escaping belong to the adapter.
    fn send(&mut self, message: Outgoing<'_>) -> std::result::Result<String, SendError>;
    fn channel(&mut self, id: &str) -> Result<ChannelInfo>;
    /// Fetch ALL pages newer than the opaque message ID; thread=None means channel history.
    /// Translate identically to live messages; edits and bot messages are excluded.
    fn history(&mut self, channel: &str, thread: Option<&str>, after: &str)
    -> Result<Vec<Message>>;
    /// Stop and join owned network workers when dropped, including on configuration reload.
    fn close(&mut self) {}
}

pub trait Chat: Sync {
    fn kind(&self) -> &'static str;
    fn secret_keys(&self) -> &'static [&'static str];
    fn config_keys(&self) -> &'static [&'static str] {
        &[]
    }
    fn read_config(&self, table: &toml::Table) -> std::result::Result<ChatConfig, Problem>;
    fn connect(&self, options: Connect) -> crate::core::Result<Box<dyn Connection>>;
}

pub static CHATS: &[&dyn Chat] = &[&slack::Slack];

#[cfg(test)]
mod tests {
    use super::CHATS;
    use std::{fs, path::Path};

    #[test]
    fn common_core_has_no_registered_adapter_names() {
        fn check(path: &Path) {
            if path.is_dir() {
                for entry in fs::read_dir(path).unwrap() {
                    check(&entry.unwrap().path());
                }
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let source = fs::read_to_string(path).unwrap();
                for adapter in CHATS {
                    assert!(
                        !source.contains(adapter.kind()),
                        "{} contains adapter name {}",
                        path.display(),
                        adapter.kind()
                    );
                }
            }
        }
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        check(&source.join("core"));
        check(&source.join("main.rs"));
    }
}
