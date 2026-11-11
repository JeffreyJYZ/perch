//! perch-store: settings, keychain secrets and the append-only session log.

pub mod secret;
pub mod session;
pub mod settings;

pub use session::{Logged, SessionEvent, SessionLog, SessionMeta, meta_path};
pub use settings::Settings;

/// Every failure perch-store can produce.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("toml decode: {0}")]
    TomlDecode(#[from] toml::de::Error),
    #[error("toml encode: {0}")]
    TomlEncode(#[from] toml::ser::Error),
    #[error("keyring: {0}")]
    Keyring(#[from] keyring::Error),
}

/// A perch-store result.
pub type Result<T> = std::result::Result<T, Error>;
