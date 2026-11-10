//! Reading and writing the settings the window edits, and the one secret stored beside them.
//!
//! The provider's address, model and theme live in `settings.toml`; the key lives in whatever
//! [`SecretStore`] the app was opened with, and never in the file. A save folds both into the
//! app's [`ProviderDefaults`], which is what a new session reads, so the next turn uses the new
//! provider without the app being restarted.

use std::path::Path;

use perch_store::Settings;
use perch_store::secret::SecretStore;
use perch_ui::SettingsDraft;
use perch_ui::shell::{KeyStatus, SettingsSnapshot};

use crate::{AppState, ProviderDefaults, Result};

/// The keychain account the provider key is stored under.
pub const KEY_ACCOUNT: &str = "provider";

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
