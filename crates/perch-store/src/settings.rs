//! Application settings, stored as one TOML file in the app data directory.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Result;

/// Every setting perch knows about.
///
/// Each field has a default so a file written by an older build still loads, and the defaults
/// are what `load` writes the first time it runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub provider_base_url: String,
    pub model: String,
    pub theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            provider_base_url: "https://openrouter.ai/api/v1".to_string(),
            model: String::new(),
            theme: "dark".to_string(),
        }
    }
}

impl Settings {
    /// Read `<dir>/settings.toml`, writing the defaults there first when it does not exist.
    pub fn load(dir: &Path) -> Result<Self> {
        match std::fs::read_to_string(path_in(dir)) {
            Ok(text) => Ok(toml::from_str(&text)?),
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
}

/// The one settings file, `<dir>/settings.toml`.
pub fn path_in(dir: &Path) -> PathBuf {
    dir.join("settings.toml")
}
