//! Following the settings file: the provider a new session reads and the tools a save rebuilds.

use perch_store::Settings;
use perch_store::settings::McpServerConfig;
use perch_ui::settings::SettingsDraft;
use perch_ui::shell::{KeyStatus, SettingsSnapshot};
use perch_ui::theme::{self, Appearance};

use super::{AppState, Error, ProviderDefaults};
use crate::update::{Plan, Worker as UpdateWorker};

impl AppState {
    /// The provider a new session would be given: what a settings save changes.
    pub fn provider_defaults(&self) -> &ProviderDefaults {
        &self.defaults
    }

    /// What the frame draws with: the theme `settings.toml` names, in the colours on disk, and the
    /// type size a person picked.
    ///
    /// Read from the file on every call rather than kept, which is what makes a save reach a
    /// window that is already open: there is no copy of the appearance to go stale, the way
    /// [`AppState::follow_saved_endpoint`] leaves no session on an old address. The note is the one
    /// line a pane says when the saved name or size could not be used as written.
    pub fn appearance(&self) -> Appearance {
        let settings = Settings::load(&self.dir).unwrap_or_default();
        Appearance::resolve(&theme::discover(), &settings.theme, settings.font_size)
    }

    /// The model the active session asks for, exactly as the composer shows it.
    pub fn model(&self) -> Option<String> {
        self.active_session()
            .map(|session| session.meta.model.clone())
    }

    /// Picks `id` for the active session only: its next turn asks for it, its log header says so,
    /// and a session that is already streaming keeps the model it was sent with.
    ///
    /// The choice also becomes what a new session inherits. It is not written to `settings.toml`:
    /// that is the settings panel's save, and a composer pick is a session's business.
    pub fn choose_model(&mut self, id: &str) -> crate::Result<()> {
        let id = id.trim();
        if id.is_empty() {
            return Err(Error::Model(
                "a model id is needed, and none was picked".to_string(),
            ));
        }
        let Some(runtime) = self.sessions.get_mut(self.active) else {
            return Err(Error::NoSession);
        };
        runtime.model.set(id);
        runtime.meta.model = id.to_string();
        runtime.log.set_model(id)?;
        self.defaults.model = id.to_string();
        Ok(())
    }

    /// What the settings panel opens with: the saved values and whether a key is stored.
    pub fn settings(&self) -> SettingsSnapshot {
        crate::settings::snapshot(&self.dir, &*self.secrets)
    }

    /// Writes the panel's draft, folds it into the provider defaults, and follows the manifest URL
    /// it names — all without a restart.
    ///
    /// A session that is already running keeps the provider it was spawned with; the next session
    /// gets these values, which is what "applies immediately" can mean without restarting a turn.
    /// The update thread is replaced only when the URL actually changed, so saving a theme does not
    /// start a network check.
    pub fn save_settings(
        &mut self,
        draft: SettingsDraft,
    ) -> std::result::Result<KeyStatus, String> {
        let status = crate::settings::apply(&self.dir, &mut self.defaults, &*self.secrets, draft)
            .map_err(|error| error.to_string())?;
        // A saved model is the session's too. Saving is an explicit act, and a window that keeps
        // sending "model is empty" after one is a window that ignored what it was just told.
        self.follow_saved_model();
        // So are the address and the key: a key added after the window opened used to reach only
        // the sessions created after it, so the one on screen kept answering `401` with a key the
        // app already held.
        self.follow_saved_endpoint();
        self.follow_update().map_err(|error| error.to_string())?;
        Ok(status)
    }

    /// Gives every session the address and key just saved, so the next turn of the open one asks
    /// the provider the person just configured.
    pub(super) fn follow_saved_endpoint(&mut self) {
        let base_url = self.defaults.base_url.clone();
        let api_key = self.defaults.api_key.clone();
        for session in &mut self.sessions {
            session.endpoint.set(&base_url, &api_key);
        }
    }

    /// Gives the active session the model that was just saved, when it is not already asking for
    /// one. Nothing is chosen when the saved model is empty, so a provider without a model yet
    /// still refuses a turn in words rather than guessing.
    pub(super) fn follow_saved_model(&mut self) {
        let saved = self.defaults.model.trim().to_string();
        if saved.is_empty() {
            return;
        }
        let differs = self
            .sessions
            .get(self.active)
            .is_some_and(|session| session.meta.model != saved);
        if differs {
            let _ = self.choose_model(&saved);
        }
    }

    /// Restarts the update thread when `settings.toml` now names a different manifest.
    ///
    /// An empty URL is no thread at all, so clearing the field stops the check as well as starting
    /// one: dropping the worker closes the channel its thread waits on, and that thread ends after
    /// the job it is in.
    fn follow_update(&mut self) -> crate::Result<()> {
        let url = Settings::load(&self.dir)?.update_check;
        if url == self.update_url {
            return Ok(());
        }
        let plan = Plan::new(&url, self.update_transport.clone())?;
        self.update =
            plan.map(|plan| UpdateWorker::start(plan.url, self.dir.clone(), plan.transport));
        self.update_url = url;
        self.update_status = self.update.as_ref().map(UpdateWorker::latest);
        Ok(())
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
        let active = settings.active_profile().cloned();
        self.defaults.base_url = active
            .as_ref()
            .map(|profile| profile.base_url.clone())
            .unwrap_or_default();
        self.defaults.model = active
            .as_ref()
            .map(|profile| profile.model.clone())
            .unwrap_or_default();
        self.defaults.api_key = active
            .as_ref()
            .and_then(|profile| crate::settings::runtime_key(&*self.secrets, &profile.name))
            .unwrap_or_default();
    }
}
