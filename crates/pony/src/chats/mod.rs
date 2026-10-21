//! Code-level adapters. Connection, events and delivery are added in R38e.
mod slack;

use crate::core::config::{ChatConfig, Problem};

pub trait Chat: Sync {
    fn kind(&self) -> &'static str;
    fn secret_keys(&self) -> &'static [&'static str];
    fn read_config(&self, table: &toml::Table) -> Result<ChatConfig, Problem>;
}

pub static CHATS: &[&dyn Chat] = &[&slack::Slack];
