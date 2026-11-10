//! Reading and writing the settings the window edits, and the one secret stored beside them.
//!
//! The provider's address, model, theme, update check URL and MCP servers live in `settings.toml`;
//! the key lives in whatever [`SecretStore`] the app was opened with, and never in the file. A save
//! folds the provider into the app's [`ProviderDefaults`], which is what a new session reads,
//! restarts the update thread when the manifest URL changed, and the MCP entries are picked up by
//! the reload the settings watcher triggers — so all of them apply without the app being restarted.

use std::path::Path;
use std::sync::Arc;

use perch_store::Settings;
use perch_store::secret::{Keychain, SecretStore};
use perch_ui::SettingsDraft;
use perch_ui::shell::{KeyStatus, SettingsSnapshot};
use perch_update::Transport;

use crate::update::Plan;
use crate::{AppState, ProviderDefaults, Result};

/// The keychain account the provider key is stored under.
pub const KEY_ACCOUNT: &str = "provider";

/// The plugin directory under the data directory: where a person drops a `*.wasm` to install one.
pub const PLUGINS: &str = "plugins";

/// Opens the app's data directory, reading the provider from settings unless it is stated.
///
/// A provider that is stated is used as given: a test hands over a fixture server's address, and
/// a scripted run hands over what a flag said. Both still get their key from `secrets`. Plugins
/// come from `<dir>/plugins`.
pub fn open(
    dir: &Path,
    stated: Option<ProviderDefaults>,
    secrets: Box<dyn SecretStore>,
) -> Result<AppState> {
    open_at(dir, stated, secrets, &dir.join(PLUGINS))
}

/// The same, with the plugin directory stated instead of `<dir>/plugins`.
pub fn open_at(
    dir: &Path,
    stated: Option<ProviderDefaults>,
    secrets: Box<dyn SecretStore>,
    plugins: &Path,
) -> Result<AppState> {
    open_at_with(dir, stated, secrets, plugins, None)
}

/// The same, with the update transport stated instead of the real one.
///
/// A stated transport is what a test hands over, so the check and the download run without a
/// socket; the real client is built only when `settings.toml` names a manifest and no transport
/// was stated.
pub(crate) fn open_at_with(
    dir: &Path,
    stated: Option<ProviderDefaults>,
    secrets: Box<dyn SecretStore>,
    plugins: &Path,
    transport: Option<Arc<dyn Transport>>,
) -> Result<AppState> {
    let settings = Settings::load(dir)?;
    let given = stated.is_some();
    let defaults = stated.unwrap_or_else(|| ProviderDefaults {
        base_url: settings.provider_base_url.clone(),
        model: settings.model.clone(),
        api_key: stored_key(&*secrets).unwrap_or_default(),
    });
    // An empty URL is no plan at all, which is what keeps an unconfigured window off the network.
    let update = Plan::new(&settings.update_check, transport)?;
    AppState::build(
        dir,
        defaults,
        given,
        &settings.mcp_servers,
        secrets,
        plugins,
        update,
    )
}

/// Opens the app for the binary: settings, keychain and a plugin directory the flags chose.
pub fn open_with_plugins(dir: &Path, plugins: &Path) -> Result<AppState> {
    open_at(dir, None, Box::new(Keychain), plugins)
}

/// What the settings panel opens with: the saved values and whether a key is stored.
///
/// A settings file that will not decode is not a closed panel: the defaults it would have had
/// are shown, so a file a person broke by hand is corrected by one save instead of a window that
/// cannot say what it thinks.
pub fn snapshot(dir: &Path, secrets: &dyn SecretStore) -> SettingsSnapshot {
    let Settings {
        provider_base_url,
        model,
        theme,
        update_check,
        ..
    } = Settings::load(dir).unwrap_or_default();
    SettingsSnapshot {
        base_url: provider_base_url,
        model,
        theme,
        update_check,
        key: status(secrets),
    }
}

/// Writes `draft` to `settings.toml`, stores the key it carries, and folds both into `defaults`.
///
/// The key is not a field of [`Settings`], so it has no path into the file: only the values
/// assigned below are written. An empty key keeps the one already stored, so saving a theme does
/// not sign the person out.
pub fn apply(
    dir: &Path,
    defaults: &mut ProviderDefaults,
    secrets: &dyn SecretStore,
    draft: SettingsDraft,
) -> Result<KeyStatus> {
    let mut settings = Settings::load(dir)?;
    settings.provider_base_url = draft.base_url;
    settings.model = draft.model;
    settings.theme = draft.theme;
    settings.update_check = draft.update_check;
    settings.save(dir)?;
    if !draft.api_key.is_empty() {
        secrets.store(KEY_ACCOUNT, &draft.api_key)?;
    }
    defaults.base_url = settings.provider_base_url;
    defaults.model = settings.model;
    defaults.api_key = stored_key(secrets).unwrap_or_default();
    Ok(status(secrets))
}

/// Whether a key is stored, as the panel says it.
fn status(secrets: &dyn SecretStore) -> KeyStatus {
    match stored_key(secrets) {
        Some(key) if !key.is_empty() => KeyStatus::Stored,
        _ => KeyStatus::Missing,
    }
}

/// The key, when the store has one. A store that refuses is read as nothing stored, because the
/// window has no way to show an error it cannot name and the next save is the repair.
pub(crate) fn stored_key(secrets: &dyn SecretStore) -> Option<String> {
    secrets.load(KEY_ACCOUNT).ok().flatten()
}
