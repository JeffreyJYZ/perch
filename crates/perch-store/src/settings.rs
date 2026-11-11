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
    /// MCP servers whose tools the model may call. Empty by default: a server is opt-in,
    /// because connecting one spawns a process or opens a browser.
    pub mcp_servers: Vec<McpServerConfig>,
    /// The URL of the release manifest this app checks at startup. Empty by default, and empty
    /// means "do not check": a window that opens must never reach the network unasked.
    pub update_check: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            provider_base_url: "https://openrouter.ai/api/v1".to_string(),
            model: String::new(),
            theme: "dark".to_string(),
            mcp_servers: Vec::new(),
            update_check: String::new(),
        }
    }
}

/// One MCP server, as written in `settings.toml`.
///
/// A server is either local (`command` plus `args`, spoken to over stdio) or remote (`url`,
/// spoken to over streamable HTTP). The `name` is what namespaces its tools, so it is
/// required and is the one thing a user must choose deliberately.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct McpServerConfig {
    pub name: String,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub url: Option<String>,
}

impl McpServerConfig {
    /// A local server spawned as a child process and spoken to over its pipes.
    pub fn stdio(name: impl Into<String>, command: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            name: name.into(),
            command: Some(command.into()),
            args,
            url: None,
        }
    }

    /// A remote server reached over streamable HTTP, with OAuth when it asks for it.
    pub fn remote(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: None,
            args: Vec::new(),
            url: Some(url.into()),
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

/// The saved permission rules, `<dir>/permissions.jsonl`.
///
/// One rule per line, so a person can read and edit it beside `settings.toml`. The path is handed
/// to the rules store rather than discovered there, because the data directory is this crate's to
/// name and nobody else's.
pub fn rules_path(dir: &Path) -> PathBuf {
    dir.join("permissions.jsonl")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_live_beside_the_settings_a_person_edits() {
        let dir = Path::new("/data/perch");
        assert_eq!(rules_path(dir), Path::new("/data/perch/permissions.jsonl"));
        assert_eq!(rules_path(dir).parent(), path_in(dir).parent());
    }

    #[test]
    fn the_update_check_is_empty_until_a_person_names_a_manifest() {
        assert_eq!(
            Settings::default().update_check,
            "",
            "the default must not reach the network"
        );
    }

    #[test]
    fn a_file_without_the_update_key_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(path_in(dir.path()), "model = \"some/model\"\n").unwrap();
        let settings = Settings::load(dir.path()).unwrap();
        assert_eq!(settings.model, "some/model");
        assert_eq!(settings.update_check, "");
    }

    #[test]
    fn a_update_check_url_written_by_hand_is_read_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let url = "https://release.perch.test/latest.json";
        std::fs::write(path_in(dir.path()), format!("update_check = \"{url}\"\n")).unwrap();

        let settings = Settings::load(dir.path()).unwrap();
        assert_eq!(
            settings.update_check, url,
            "the field is read like the others"
        );

        settings.save(dir.path()).unwrap();
        assert_eq!(
            Settings::load(dir.path()).unwrap().update_check,
            url,
            "a save keeps the URL it did not edit"
        );
    }
}
