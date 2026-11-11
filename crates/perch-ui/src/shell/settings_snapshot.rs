//! What the settings panel opens with, and the word it shows in place of the key.
//!
//! The key itself never reaches the window: the panel shows `missing` or `stored`, and the value is
//! the store's business.

use crate::settings::DEFAULT_THEME;

/// Whether a provider key is stored, as the settings panel says it.
///
/// The key itself is never part of this: the panel shows one of these words instead, so a saved
/// secret has no path back to the screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeyStatus {
    /// Nothing is stored for the provider account.
    #[default]
    Missing,
    /// A key is stored; its value is the keychain's business, not the window's.
    Stored,
}

impl KeyStatus {
    /// The word the panel shows in place of the key.
    pub fn words(self) -> &'static str {
        match self {
            KeyStatus::Missing => "missing",
            KeyStatus::Stored => "stored",
        }
    }
}

/// What the settings panel opens with: the saved provider settings and whether a key is stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsSnapshot {
    /// The provider's API root.
    pub base_url: String,
    /// The model every new session asks for.
    pub model: String,
    /// The saved theme.
    pub theme: String,
    /// The saved update check URL, empty when the app checks nothing.
    pub update_check: String,
    /// Whether a key is stored for the provider account.
    pub key: KeyStatus,
}

impl Default for SettingsSnapshot {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            model: String::new(),
            theme: DEFAULT_THEME.to_string(),
            update_check: String::new(),
            key: KeyStatus::Missing,
        }
    }
}
