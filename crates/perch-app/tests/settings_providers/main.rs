//! Several providers and one active: the list, the switch, and the migration that keeps an old
//! machine's provider and its key.
//!
//! Every app here is opened over an [`MemorySecrets`], so no test touches the real store, and
//! every one works in a throwaway data directory: the machine the suite runs on is never read.

#[path = "../support/mod.rs"]
mod support;

use std::sync::Arc;

use perch_app::{AppState, ProviderDefaults};
use perch_store::secret::MemorySecrets;
use perch_ui::SettingsDraft;

use support::hermetic;

mod migration;
mod switch;

/// A draft that edits the active profile.
pub fn draft(base_url: &str, model: &str, api_key: &str) -> SettingsDraft {
    SettingsDraft {
        base_url: base_url.to_string(),
        model: model.to_string(),
        theme: "dark".to_string(),
        api_key: api_key.to_string(),
        ..Default::default()
    }
}

/// A draft that edits the profile named `profile`.
pub fn profile_draft(profile: &str, base_url: &str, model: &str, api_key: &str) -> SettingsDraft {
    SettingsDraft {
        profile: profile.to_string(),
        ..draft(base_url, model, api_key)
    }
}

/// A draft that edits the profile named `profile` and makes it the active one.
pub fn activate_draft(profile: &str, base_url: &str, model: &str) -> SettingsDraft {
    SettingsDraft {
        activate: true,
        ..profile_draft(profile, base_url, model, "")
    }
}

/// An app over `data` that keeps its keys in memory.
pub fn open(data: &tempfile::TempDir, secrets: &Arc<MemorySecrets>) -> AppState {
    hermetic();
    AppState::open_with_secrets(
        data.path(),
        ProviderDefaults::default(),
        Box::new(Arc::clone(secrets)),
    )
    .expect("a data directory with no sessions opens")
}

/// An app over `data` that reads the provider from the file, the way the window does.
pub fn open_reading_the_file(data: &tempfile::TempDir, secrets: &Arc<MemorySecrets>) -> AppState {
    hermetic();
    perch_app::settings::open(data.path(), None, Box::new(Arc::clone(secrets)))
        .expect("a data directory with no sessions opens")
}
