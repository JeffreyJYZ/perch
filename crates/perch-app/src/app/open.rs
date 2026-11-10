//! The constructors a window, a test or a script opens an app with.

use std::path::Path;
use std::sync::Arc;

use perch_store::secret::{self, SecretStore};
use perch_update::Transport;

use super::{AppState, ProviderDefaults, Result};

impl AppState {
    /// Opens every session whose log is in `dir`, reading the provider from settings and the key
    /// from the keychain.
    ///
    /// A keychain that refuses is not fatal: the window opens and a turn reports the provider's
    /// rejection, which is a better failure than a window that will not start.
    pub fn open(dir: &Path) -> Result<AppState> {
        crate::settings::open(dir, None, Box::new(secret::Keychain))
    }

    /// The same, with the provider stated instead of read: what a test or a scripted run uses.
    ///
    /// MCP servers still come from settings and plugins from `<dir>/plugins`, so a session started
    /// this way sees the tools the window would give it.
    pub fn open_with(dir: &Path, defaults: ProviderDefaults) -> Result<AppState> {
        crate::settings::open(dir, Some(defaults), Box::new(secret::Keychain))
    }

    /// The same, with the secret store stated: what a test uses so it never reaches the keychain.
    pub fn open_with_secrets(
        dir: &Path,
        defaults: ProviderDefaults,
        secrets: Box<dyn SecretStore>,
    ) -> Result<AppState> {
        crate::settings::open(dir, Some(defaults), secrets)
    }

    /// The same as [`AppState::open_with_secrets`], with the plugin directory stated too.
    pub fn open_at(
        dir: &Path,
        defaults: ProviderDefaults,
        secrets: Box<dyn SecretStore>,
        plugins: &Path,
    ) -> Result<AppState> {
        crate::settings::open_at(dir, Some(defaults), secrets, plugins)
    }

    /// The same, with the update transport stated: what a test uses so no socket is opened.
    ///
    /// The manifest URL still comes from `settings.toml` and the plugins from `<dir>/plugins`, so
    /// the test exercises the same reading the window does — including the empty default that
    /// means no check at all.
    pub fn open_with_update(
        dir: &Path,
        defaults: ProviderDefaults,
        secrets: Box<dyn SecretStore>,
        transport: Arc<dyn Transport>,
    ) -> Result<AppState> {
        crate::settings::open_at_with(
            dir,
            Some(defaults),
            secrets,
            &dir.join(crate::settings::PLUGINS),
            Some(transport),
        )
    }
}
