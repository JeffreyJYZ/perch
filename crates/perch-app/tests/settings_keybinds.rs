//! The keybinds as `settings.toml` keeps them: the table a save writes, and the chords a reopen
//! reads back.
//!
//! Every app here is opened in a throwaway data directory over an in-memory secret store, so no
//! test reads the machine it runs on.

mod support;

use std::collections::BTreeMap;
use std::sync::Arc;

use perch_app::settings::account_for;
use perch_app::{AppState, ProviderDefaults};
use perch_store::Settings;
use perch_store::secret::{MemorySecrets, SecretStore};
use perch_ui::SettingsDraft;
use perch_ui::shell::Bindable;

use support::hermetic;

/// A draft that saves the working provider and names `keybinds` as the chords to store.
fn draft(keybinds: &[(&str, &str)]) -> SettingsDraft {
    let mut binds = BTreeMap::new();
    for (intent, chord) in keybinds {
        binds.insert(intent.to_string(), chord.to_string());
    }
    SettingsDraft {
        base_url: "https://example.com/v1".to_string(),
        model: "test/model".to_string(),
        theme: "dark".to_string(),
        api_key: "sk-test".to_string(),
        keybinds: binds,
        ..Default::default()
    }
}

/// An app over `data` that keeps its key in memory.
fn open(data: &tempfile::TempDir, secrets: &Arc<MemorySecrets>) -> AppState {
    hermetic();
    AppState::open_with_secrets(
        data.path(),
        ProviderDefaults::default(),
        Box::new(Arc::clone(secrets)),
    )
    .expect("a data directory with no sessions opens")
}

#[test]
fn a_typed_chord_is_written_to_the_keybinds_table() {
    let data = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecrets::new());
    let mut app = open(&data, &secrets);

    app.save_settings(draft(&[("settings", "mod+j")])).unwrap();

    let text = std::fs::read_to_string(perch_store::settings::path_in(data.path())).unwrap();
    let shaped = "active_provider = \"default\"\ntheme = \"dark\"\nupdate_check = \"\"\n\
                  mcp_servers = []\n\n[keybinds]\nsettings = \"mod+j\"\n\n[[providers]]\n\
                  name = \"default\"\nbase_url = \"https://example.com/v1\"\n\
                  model = \"test/model\"\n";
    assert_eq!(
        text, shaped,
        "the file is the shape every later read expects"
    );
    assert!(text.contains("settings = \"mod+j\""), "{text}");
    assert_eq!(
        Settings::load(data.path())
            .unwrap()
            .keybinds
            .get("settings"),
        Some(&"mod+j".to_string()),
        "the table round-trips"
    );
    assert_eq!(
        app.settings().keybinds.chord(Bindable::Settings),
        "mod+j",
        "and the pane resolves the saved chord over the default"
    );
    assert_eq!(
        app.settings().keybinds.chord(Bindable::Palette),
        "mod+k",
        "an intent nobody bound keeps its default"
    );
}

#[test]
fn a_save_that_types_no_chord_writes_no_keybinds_table() {
    let data = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecrets::new());
    let mut app = open(&data, &secrets);

    app.save_settings(draft(&[])).unwrap();

    let text = std::fs::read_to_string(perch_store::settings::path_in(data.path())).unwrap();
    assert!(
        !text.contains("keybinds"),
        "an untouched table is not written at all:\n{text}"
    );
    assert_eq!(
        app.settings().keybinds.chord(Bindable::Settings),
        "mod+,",
        "and the window answers the chord it always has"
    );
}

#[test]
fn a_saved_chord_is_read_back_by_a_reopen() {
    let data = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecrets::new());
    let mut app = open(&data, &secrets);
    app.save_settings(draft(&[("palette", "ctrl+shift+p")]))
        .unwrap();

    let reopened = open(&data, &secrets);
    let bindings = reopened.settings().keybinds;

    assert_eq!(
        bindings.chord(Bindable::Palette),
        "ctrl+shift+p",
        "the chord a restart reads is the one that was saved"
    );
    assert!(
        bindings.refusal_words().is_empty(),
        "and nothing about it is refused"
    );
}

#[test]
fn an_intent_this_window_does_not_know_is_kept_as_it_was() {
    let data = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecrets::new());
    let mut settings = Settings::default();
    settings
        .keybinds
        .insert("nonsense".to_string(), "mod+j".to_string());
    settings.save(data.path()).unwrap();
    let mut app = open(&data, &secrets);

    app.save_settings(draft(&[])).unwrap();

    assert_eq!(
        Settings::load(data.path())
            .unwrap()
            .keybinds
            .get("nonsense"),
        Some(&"mod+j".to_string()),
        "a name this window cannot use is not its to throw away"
    );
    assert!(
        !app.settings().keybinds.refusal_words().is_empty(),
        "and the pane says it could not be used"
    );
    // The secrets store is asserted so the account name is pinned: a keybind save also stores a
    // key, and the account it lands in is the active profile's.
    assert!(secrets.load(&account_for("default")).unwrap().is_some());
}
