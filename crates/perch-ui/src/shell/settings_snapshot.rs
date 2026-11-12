//! What the settings panel opens with, and the word it shows in place of the key.
//!
//! The key itself never reaches the window: the panel shows `missing` or `stored`, and the value is
//! the store's business.

use crate::settings::DEFAULT_THEME;

use super::keybind::Bindings;

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

/// One provider profile as the settings panel lists it.
///
/// The key is a word, never a value: a saved secret has no path back to the screen.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProviderView {
    /// The name a person gave the profile, and the account its key is filed under.
    pub name: String,
    /// The provider's API root.
    pub base_url: String,
    /// The model a turn asks for.
    pub model: String,
    /// Whether a key is stored for this profile.
    pub key: KeyStatus,
    /// Whether this profile is the active one: the provider a new turn asks.
    pub active: bool,
}

/// What the settings panel opens with: the saved provider settings and whether a key is stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsSnapshot {
    /// The active provider's API root.
    pub base_url: String,
    /// The model every new session asks for.
    pub model: String,
    /// The saved theme.
    pub theme: String,
    /// The saved type scale in points, empty when the default scale is drawn.
    pub font_size: String,
    /// The saved update check URL, empty when the app checks nothing.
    pub update_check: String,
    /// Whether a key is stored for the active provider.
    pub key: KeyStatus,
    /// The provider profiles the settings file holds.
    pub profiles: Vec<ProviderView>,
    /// The chords the window answers to, over the defaults.
    pub keybinds: Bindings,
}

impl Default for SettingsSnapshot {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            model: String::new(),
            theme: DEFAULT_THEME.to_string(),
            font_size: String::new(),
            update_check: String::new(),
            key: KeyStatus::Missing,
            profiles: Vec::new(),
            keybinds: Bindings::defaults(),
        }
    }
}

impl SettingsSnapshot {
    /// The active profile, when the list holds one.
    pub fn active_profile(&self) -> Option<&ProviderView> {
        self.profiles.iter().find(|profile| profile.active)
    }
}
