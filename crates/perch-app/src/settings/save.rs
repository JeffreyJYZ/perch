//! Writing the settings the window edited.
//!
//! The save is one file and one secret: the edited profile goes into `settings.toml`, and the key
//! it carries goes into the secret store under that profile's name.

use std::collections::BTreeMap;
use std::path::Path;

use perch_store::Settings;
use perch_store::secret::SecretStore;
use perch_ui::SettingsDraft;
use perch_ui::shell::KeyStatus;

use super::profiles;
use crate::{ProviderDefaults, Result};

/// Writes `draft` to `settings.toml`, stores the key it carries, and folds both into `defaults`.
///
/// The key is not a field of [`Settings`], so it has no path into the file: only the values
/// assigned below are written. An empty key keeps the one already stored, so saving a theme does
/// not sign the person out.
///
/// The draft names the profile it edits, and the profile named is the one the panel had selected.
/// A draft that names none edits the active profile, which is what every save from before the
/// list existed did.
pub fn apply(
    dir: &Path,
    defaults: &mut ProviderDefaults,
    secrets: &dyn SecretStore,
    draft: SettingsDraft,
) -> Result<KeyStatus> {
    // The typed size, clamped to the scale's own bounds by `font_points`, so a size of 8 is saved
    // as the 10 the pane says it draws, and an empty field is the default scale. Read before the
    // fields below move out of the draft.
    let font_size = draft.font_points();
    let mut settings = Settings::load(dir)?;
    settings.theme = draft.theme.clone();
    settings.font_size = font_size;
    settings.update_check = draft.update_check.clone();

    let named = draft.profile.trim();
    let named = if named.is_empty() {
        settings.active_provider.clone()
    } else {
        named.to_string()
    };
    let base_url = draft.base_url.trim().to_string();
    let model = draft.model.trim().to_string();
    let name = match settings.profile_mut(&named) {
        Some(profile) => {
            profile.base_url = base_url;
            profile.model = model;
            profile.name.clone()
        }
        None => {
            // A name the list does not carry is a profile being added.
            settings
                .providers
                .push(perch_store::settings::ProviderProfile {
                    name: named.clone(),
                    base_url,
                    model,
                });
            named
        }
    };
    if !draft.api_key.trim().is_empty() {
        secrets.store(&profiles::account_for(&name), draft.api_key.trim())?;
    }
    settings.keybinds = keybinds(&settings.keybinds, &draft.keybinds);
    if draft.activate {
        settings.active_provider = name.clone();
    }
    settings.save(dir)?;

    // The defaults follow the profile that is active now, so the next turn asks it.
    if let Some(active) = settings.active_profile().cloned() {
        defaults.base_url = active.base_url.clone();
        defaults.model = active.model.clone();
        defaults.api_key = profiles::runtime_key(secrets, &active.name).unwrap_or_default();
    }
    Ok(profiles::key_status(secrets, &name))
}

/// The chords a save writes: the ones the person typed, over the ones that were saved.
///
/// An intent the person left alone keeps the chord it had, so a pane that lists every intent
/// does not drop the ones it did not show. An intent this window does not know is kept as it
/// was: it is not this window's to throw away.
fn keybinds(
    saved: &BTreeMap<String, String>,
    typed: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut binds = saved.clone();
    for intent in perch_ui::shell::Bindable::ALL {
        match typed.get(intent.name()) {
            Some(chord) if !chord.trim().is_empty() => {
                binds.insert(intent.name().to_string(), chord.trim().to_string());
            }
            _ => {
                binds.remove(intent.name());
            }
        }
    }
    binds
}
