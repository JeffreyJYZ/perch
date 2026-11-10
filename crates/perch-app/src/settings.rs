//! Reading and writing the settings the window edits, and the one secret stored beside them.
//!
//! The provider's address, model and theme live in `settings.toml`; the key lives in whatever
//! [`SecretStore`] the app was opened with, and never in the file. A save folds both into the
//! app's [`ProviderDefaults`], which is what a new session reads, so the next turn uses the new
//! provider without the app being restarted.

use std::path::Path;

use perch_agent::Dispatcher;
use perch_plugins::Loaded;
use perch_store::Settings;
use perch_store::secret::SecretStore;
use perch_ui::SettingsDraft;
use perch_ui::shell::{KeyStatus, Panels, SettingsSnapshot};

use crate::{AppState, ProviderDefaults, Result};

/// The keychain account the provider key is stored under.
pub const KEY_ACCOUNT: &str = "provider";

/// The plugin directory under the data directory: where a person drops a `*.wasm` to install one.
pub const PLUGINS: &str = "plugins";

/// Opens the app's data directory, reading the provider from settings unless it is stated.
///
/// A provider that is stated is used as given: a test hands over a fixture server's address, and
/// a scripted run hands over what a flag said. Both still get their key from `secrets`.
pub fn open(
    dir: &Path,
    stated: Option<ProviderDefaults>,
    secrets: Box<dyn SecretStore>,
) -> Result<AppState> {
    let settings = Settings::load(dir)?;
    let defaults = stated.unwrap_or_else(|| ProviderDefaults {
        base_url: settings.provider_base_url.clone(),
        model: settings.model.clone(),
        api_key: stored_key(&*secrets).unwrap_or_default(),
    });
    AppState::build(dir, defaults, &settings.mcp_servers, secrets)
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
        ..
    } = Settings::load(dir).unwrap_or_default();
    SettingsSnapshot {
        base_url: provider_base_url,
        model,
        theme,
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
    settings.save(dir)?;
    if !draft.api_key.is_empty() {
        secrets.store(KEY_ACCOUNT, &draft.api_key)?;
    }
    defaults.base_url = settings.provider_base_url;
    defaults.model = settings.model;
    defaults.api_key = stored_key(secrets).unwrap_or_default();
    Ok(status(secrets))
}

/// Loads every plugin in `dir` into a registry built exactly as the app builds its own: the six
/// built-ins, `ask_user`, and then one tool per declared plugin tool.
///
/// This is the whole load path, and it cannot fail: a directory that is not there is no plugins,
/// and a component that will not load is one line of words that the window's plugin column says
/// out loud. A plugin a person installed must never be the reason the window does not open.
pub fn plugins(dir: &Path) -> Loaded {
    Loaded::open(dir, Dispatcher::builtin_registry())
}

/// Hands the active session's new events to the plugins, and reports whether anything moved.
///
/// The app has no event tap to subscribe to — its pump folds bridge events into the transcript and
/// drops them — but it does append every one of them to the session's log, flushing each line. So
/// the log is the feed: [`Loaded::sync`] reads the bytes it has not read and dispatches exactly
/// those events, once, in the app's own order.
pub fn feed(plugins: &mut Loaded, app: &AppState) -> bool {
    app.active_session()
        .is_some_and(|session| plugins.sync(&session.log))
}

/// The bare app draws no plugin column: the window holds the plugin host beside the app, because a
/// session's tool list is fixed when the session starts (see `window.rs`). A shell laid out over
/// `AppState` alone is a test's shell, and it correctly has nothing to show.
impl Panels for AppState {}

/// Whether a key is stored, as the panel says it.
fn status(secrets: &dyn SecretStore) -> KeyStatus {
    match stored_key(secrets) {
        Some(key) if !key.is_empty() => KeyStatus::Stored,
        _ => KeyStatus::Missing,
    }
}

/// The key, when the store has one. A store that refuses is read as nothing stored, because the
/// window has no way to show an error it cannot name and the next save is the repair.
fn stored_key(secrets: &dyn SecretStore) -> Option<String> {
    secrets.load(KEY_ACCOUNT).ok().flatten()
}
