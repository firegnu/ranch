//! Code-level adapters. Connection, events and delivery are added in R38e.
mod slack;

use crate::core::config::{ChatConfig, Problem};

pub trait Chat: Sync {
    fn kind(&self) -> &'static str;
    fn secret_keys(&self) -> &'static [&'static str];
    fn config_keys(&self) -> &'static [&'static str] {
        &[]
    }
    fn read_config(&self, table: &toml::Table) -> Result<ChatConfig, Problem>;
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
