use super::{
    ChannelInfo, Chat, Connect, Connection, HistoryAfter, Limits, Message, Outgoing, SendError,
};
use crate::core::config::{ChatConfig, Problem};

pub struct Slack;

impl Chat for Slack {
    fn connect(&self, options: Connect) -> crate::core::Result<Box<dyn Connection>> {
        (options.credentials)()?;
        Ok(Box::new(Placeholder))
    }
    fn kind(&self) -> &'static str {
        "slack"
    }

    fn secret_keys(&self) -> &'static [&'static str] {
        &["app_token", "bot_token"]
    }

    fn read_config(&self, table: &toml::Table) -> Result<ChatConfig, Problem> {
        let config = ChatConfig::parse(self.kind(), table)?;
        if !id(&config.owner, b"UW") {
            return Err(Problem::new(
                "owner",
                "expected a member ID starting with U or W, followed by uppercase ASCII letters or digits",
            ));
        }
        for (i, channel) in config.channels.iter().enumerate() {
            if !id(&channel.id, b"CG") {
                return Err(Problem::new(
                    format!("channels[{i}].id"),
                    "expected a channel ID starting with C or G, followed by uppercase ASCII letters or digits",
                ));
            }
        }
        Ok(config)
    }
}

struct Placeholder;
impl Connection for Placeholder {
    fn name(&self) -> &str {
        "Slack"
    }
    fn limits(&self) -> Limits {
        Limits {
            characters: 10_000,
            threads: true,
        }
    }
    fn send(&mut self, _: Outgoing<'_>) -> Result<String, SendError> {
        Err(SendError {
            code: "connecting".into(),
            resize: false,
        })
    }
    fn channel(&mut self, _: &str) -> crate::core::Result<ChannelInfo> {
        Err(crate::core::Error::new(
            "connecting",
            "adapter is not connected",
        ))
    }
    fn history(&mut self, _: &str, _: &HistoryAfter) -> crate::core::Result<Vec<Message>> {
        Err(crate::core::Error::new(
            "connecting",
            "adapter is not connected",
        ))
    }
    fn exists(&mut self, _: &str, _: Option<&str>, _: &str) -> crate::core::Result<bool> {
        Err(crate::core::Error::new(
            "connecting",
            "adapter is not connected",
        ))
    }
}

fn id(value: &str, prefixes: &[u8]) -> bool {
    let bytes = value.as_bytes();
    bytes.len() > 1
        && prefixes.contains(&bytes[0])
        && bytes[1..]
            .iter()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}
