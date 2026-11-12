//! Application settings, stored as one TOML file in the app data directory.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Result;

mod keybinds;
mod mcp;
mod profiles;

pub use keybinds::Chord;
pub use mcp::McpServerConfig;
pub use profiles::{DEFAULT_PROFILE, ProviderProfile};

/// Every setting perch knows about.
///
/// Each field has a default so a file written by an older build still loads, and the defaults
/// are what `load` writes the first time it runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The provider profile a new turn asks.
    pub active_provider: String,
    /// One of the theme names the window can draw.
    pub theme: String,
    /// The size the window's type scale is drawn at, in points, when a person picked one.
    ///
    /// `None` is the app's own scale, byte for byte, which is why it is not a number here: a
    /// default that happened to equal today's body size would still scale every other size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f32>,
    /// The URL of the release manifest this app checks at startup. Empty by default, and empty
    /// means "do not check": a window that opens must never reach the network unasked.
    pub update_check: String,
    /// The chords a person rebound, by intent name: `settings = "mod+,"`.
    ///
    /// Empty by default, and skipped when empty, so a file nobody touched stays byte for byte
    /// what it was.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub keybinds: BTreeMap<String, String>,
    /// MCP servers whose tools the model may call. Empty by default: a server is opt-in,
    /// because connecting one spawns a process or opens a browser.
    pub mcp_servers: Vec<McpServerConfig>,
    /// The provider profiles: a named list, of which one is active.
    pub providers: Vec<ProviderProfile>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            active_provider: DEFAULT_PROFILE.to_string(),
            theme: "dark".to_string(),
            font_size: None,
            update_check: String::new(),
            keybinds: BTreeMap::new(),
            mcp_servers: Vec::new(),
            providers: vec![ProviderProfile::default()],
        }
    }
}

impl Settings {
    /// Read `<dir>/settings.toml`, writing the defaults there first when it does not exist.
    ///
    /// A file written before the provider list existed is read as a one-entry list and rewritten
    /// in the new shape, so the file on disk matches what the app believes after one open.
    pub fn load(dir: &Path) -> Result<Self> {
        match std::fs::read_to_string(path_in(dir)) {
            Ok(text) => {
                let (settings, migrated) = Self::parse(&text)?;
                if migrated {
                    // Best effort: a directory that cannot be written still opens the app.
                    let _ = settings.save(dir);
                }
                Ok(settings)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let settings = Self::default();
                settings.save(dir)?;
                Ok(settings)
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Write these settings to `<dir>/settings.toml`, creating the directory.
    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(path_in(dir), toml::to_string_pretty(self)?)?;
        Ok(())
    }

    /// Reads a settings file, and says whether it was written before the provider list existed.
    fn parse(text: &str) -> Result<(Self, bool)> {
        let table: toml::Table = text.parse()?;
        let mut settings: Self = toml::from_str(text)?;
        let migrated = profiles::migrate_legacy(&table, &mut settings);
        if settings.providers.is_empty() {
            // A list with nothing in it is a file a person emptied by hand: the default profile
            // goes back, so the app always has a provider to ask.
            settings.providers.push(ProviderProfile::default());
            settings.active_provider = DEFAULT_PROFILE.to_string();
        }
        Ok((settings, migrated))
    }
}

/// The one settings file, `<dir>/settings.toml`.
pub fn path_in(dir: &Path) -> PathBuf {
    dir.join("settings.toml")
}

/// The saved permission rules, `<dir>/permissions.jsonl`.
///
/// One rule per line, so a person can read and edit it beside `settings.toml`. The path is handed
/// to the rules store rather than discovered there, because the data directory is this crate's to
/// name and nobody else's.
pub fn rules_path(dir: &Path) -> PathBuf {
    dir.join("permissions.jsonl")
}

#[cfg(test)]
mod tests;
