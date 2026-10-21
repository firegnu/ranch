pub mod config;
pub mod engine;
mod files;
mod lock;
mod log;
pub mod runtime;
pub mod secrets;
pub mod split;
pub mod state;
pub mod status;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Error {
    pub code: String,
    pub message: String,
}

impl Error {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
