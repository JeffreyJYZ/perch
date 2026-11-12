//! Reading and writing the settings the window edits, and the keys stored beside them.
//!
//! The provider profiles live in `settings.toml`; each profile's key lives in the secret store,
//! filed under the profile's name, and never in the file. A save folds the edited profile into
//! the app's [`ProviderDefaults`], which is what a new session reads, restarts the update thread
//! when the manifest URL changed, and the MCP entries are picked up by the reload the settings
//! watcher triggers — so all of them apply without the app being restarted.

use std::path::Path;
use std::sync::Arc;

use perch_store::Settings;
use perch_store::secret::SecretStore;
use perch_ui::shell::SettingsSnapshot;
use perch_update::Transport;

use crate::update::Plan;
use crate::{AppState, ProviderDefaults, Result};

mod profiles;
mod save;

pub(crate) use profiles::runtime_key;
pub use profiles::{account_for, first_env};
pub use save::apply;

/// The account the provider key was filed under before the profiles existed.
///
/// A file written back then still keeps its key here, and the profile it became inherits it, so
/// the migration never loses a secret.
pub const KEY_ACCOUNT: &str = "provider";

/// The plugin directory under the data directory: where a person drops a `*.wasm` to install one.
pub const PLUGINS: &str = "plugins";

/// The environment variables a provider key is read from, most specific first.
pub const KEY_VARS: [&str; 4] = [
    "PERCH_API_KEY",
    "OPENROUTER_API_KEY",
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY",
];

/// The environment variables a provider address is read from, most specific first.
pub const URL_VARS: [&str; 3] = ["PERCH_BASE_URL", "OPENROUTER_BASE_URL", "OPENAI_BASE_URL"];

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
    // A key an older file kept under the old account belongs to the profile that file became.
    profiles::adopt_legacy_key(&settings, &*secrets);
    let given = stated.is_some();
    let defaults = stated.unwrap_or_else(|| {
        let active = settings.active_profile();
        let url = settings.active_base_url();
        ProviderDefaults {
            // An address exported in the shell is used when none was saved, which is how every
            // other harness is pointed at a provider.
            base_url: if url.trim().is_empty() {
                first_env(&URL_VARS).unwrap_or_default()
            } else {
                url
            },
            model: settings.active_model(),
            api_key: active
                .and_then(|profile| profiles::runtime_key(&*secrets, &profile.name))
                .unwrap_or_default(),
        }
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
    open_at(
        dir,
        None,
        Box::new(crate::secrets::Secrets::at(dir)),
        plugins,
    )
}

/// What the settings panel opens with: the saved values and whether a key is stored.
///
/// A settings file that will not decode is not a closed panel: the defaults it would have had
/// are shown, so a file a person broke by hand is corrected by one save instead of a window that
/// cannot say what it thinks.
pub fn snapshot(dir: &Path, secrets: &dyn SecretStore) -> SettingsSnapshot {
    let settings = Settings::load(dir).unwrap_or_default();
    let active = settings.active_profile();
    let name = active
        .map(|profile| profile.name.clone())
        .unwrap_or_default();
    SettingsSnapshot {
        base_url: active
            .map(|profile| profile.base_url.clone())
            .unwrap_or_default(),
        model: active
            .map(|profile| profile.model.clone())
            .unwrap_or_default(),
        theme: settings.theme.clone(),
        font_size: settings
            .font_size
            .map(|size| size.to_string())
            .unwrap_or_default(),
        update_check: settings.update_check.clone(),
        shell: settings.shell.clone(),
        key: profiles::key_status(secrets, &name),
        profiles: profiles::views(&settings, secrets),
        keybinds: perch_ui::shell::Bindings::resolved(&settings.keybinds),
    }
}
