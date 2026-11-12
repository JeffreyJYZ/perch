//! A settings save and its model: the session already on screen asks for it, with no restart.
//!
//! Kept apart from `settings.rs` so each file states one thing, and the key store is in memory so
//! no keychain is involved.

use std::sync::Arc;

use perch_app::{AppState, ProviderDefaults};
use perch_store::secret::MemorySecrets;
use perch_ui::SettingsDraft;
use tempfile::TempDir;

/// The draft a save writes: a working provider with `model`.
fn draft(model: &str) -> SettingsDraft {
    SettingsDraft {
        base_url: "https://example.com/v1".to_string(),
        model: model.to_string(),
        theme: "dark".to_string(),
        font_size: String::new(),
        update_check: String::new(),
        api_key: String::new(),
        profile: String::new(),
        activate: false,
        keybinds: Default::default(),
    }
}

/// An app over `data` that keeps its key in memory.
fn open(data: &TempDir) -> AppState {
    AppState::open_with_secrets(
        data.path(),
        ProviderDefaults::default(),
        Box::new(Arc::new(MemorySecrets::new())),
    )
    .expect("a data directory with no sessions opens")
}

#[test]
fn a_saved_model_reaches_the_session_that_is_already_open() {
    let data = tempfile::tempdir().unwrap();
    let mut app = open(&data);
    let worktree = tempfile::tempdir().unwrap();
    app.new_session(worktree.path()).unwrap();

    app.save_settings(draft("test/model")).unwrap();

    assert_eq!(
        app.model().as_deref(),
        Some("test/model"),
        "the session already on screen asks for the saved model without being reopened"
    );
}

#[test]
fn a_key_exported_in_the_shell_is_used_when_none_was_saved() {
    // SAFETY: `PERCH_API_KEY` is read only here, and this test owns the name for its own duration.
    // Every other test hands over a `MemorySecrets` holding a key, so none of them reaches the
    // environment fallback this test is about.
    unsafe { std::env::set_var("PERCH_API_KEY", "sk-from-shell") };
    let data = tempfile::tempdir().unwrap();

    let app = AppState::open(data.path()).expect("a data directory opens with no key file");

    assert_eq!(
        app.provider_defaults().api_key,
        "sk-from-shell",
        "a key exported in the shell is the one a harness uses, with no panel involved"
    );
    unsafe { std::env::remove_var("PERCH_API_KEY") };
}
