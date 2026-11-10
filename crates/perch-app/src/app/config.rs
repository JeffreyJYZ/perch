//! Following the settings file: the provider a new session reads and the tools a save rebuilds.

use perch_store::Settings;
use perch_store::settings::McpServerConfig;
use perch_ui::settings::SettingsDraft;
use perch_ui::shell::{KeyStatus, SettingsSnapshot};

use super::{AppState, ProviderDefaults};

impl AppState {
    /// The provider a new session would be given: what a settings save changes.
    pub fn provider_defaults(&self) -> &ProviderDefaults {
        &self.defaults
    }

    /// What the settings panel opens with: the saved values and whether a key is stored.
    pub fn settings(&self) -> SettingsSnapshot {
        crate::settings::snapshot(&self.dir, &*self.secrets)
    }

    /// Writes the panel's draft and folds it into the provider defaults, with no restart.
    ///
    /// A session that is already running keeps the provider it was spawned with; the next session
    /// gets these values, which is what "applies immediately" can mean without restarting a turn.
    pub fn save_settings(
        &mut self,
        draft: SettingsDraft,
    ) -> std::result::Result<KeyStatus, String> {
        crate::settings::apply(&self.dir, &mut self.defaults, &*self.secrets, draft)
            .map_err(|error| error.to_string())
    }

    /// Rebuilds the tools from disk and swaps them in: the plugin directory, `settings.toml`, and
    /// the MCP servers it names.
    ///
    /// The rebuild starts from an empty registry, so what comes back is exactly what is on disk —
    /// a deleted plugin has nothing left to resurrect it. It runs on the worker, so this call may
    /// wait for a server that has to be spawned while no frame ever does; the answer is what a
    /// save and a test need, and the watcher uses [`AppState::reload_soon`] instead. A turn
    /// already in flight is untouched: its calls read the registry when they start, so the next
    /// one sees the new list. Answers whether the settings file could be read; a file a person
    /// broke by hand leaves the tools alone rather than losing the ones that work.
    pub fn reload(&mut self) -> bool {
        let Some(servers) = self.settings_servers() else {
            return false;
        };
        self.worker.rebuild(servers);
        self.absorb();
        true
    }

    /// The same rebuild, asked for and not waited for: what a frame does when the watcher fires.
    ///
    /// A save that names a new server costs the worker the handshake and costs this frame
    /// nothing. Answers whether the settings file could be read, so a broken one says so.
    pub fn reload_soon(&mut self) -> bool {
        let Some(servers) = self.settings_servers() else {
            return false;
        };
        self.worker.rebuild_soon(servers);
        true
    }

    /// The servers to rebuild against, after folding the settings file's provider in.
    fn settings_servers(&mut self) -> Option<Vec<McpServerConfig>> {
        let settings = Settings::load(&self.dir).ok()?;
        if !self.stated {
            self.follow(&settings);
        }
        Some(settings.mcp_servers)
    }

    /// Folds the settings file's provider into the defaults a new session would read.
    fn follow(&mut self, settings: &Settings) {
        self.defaults.base_url = settings.provider_base_url.clone();
        self.defaults.model = settings.model.clone();
        self.defaults.api_key = crate::settings::stored_key(&*self.secrets).unwrap_or_default();
    }
}
