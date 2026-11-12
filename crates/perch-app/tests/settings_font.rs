//! The font size as the settings pane saves it: clamped to the scale's bounds, written to
//! `settings.toml`, and read back by the pane that opened over it.

use std::sync::Arc;

use perch_app::{AppState, ProviderDefaults};
use perch_store::Settings;
use perch_store::secret::MemorySecrets;
use perch_ui::SettingsDraft;

/// The draft a save writes, with `font_size` as the one field a test varies.
fn draft(font_size: &str) -> SettingsDraft {
    SettingsDraft {
        base_url: "https://example.com/v1".to_string(),
        model: "test/model".to_string(),
        theme: "dark".to_string(),
        font_size: font_size.to_string(),
        update_check: String::new(),
        shell: String::new(),
        api_key: String::new(),
        profile: String::new(),
        activate: false,
        keybinds: Default::default(),
    }
}

/// The app over `data`, with its key in memory rather than the login keychain.
fn open(data: &tempfile::TempDir) -> AppState {
    AppState::open_with_secrets(
        data.path(),
        ProviderDefaults::default(),
        Box::new(Arc::new(MemorySecrets::new())),
    )
    .expect("a data directory with no sessions opens")
}

#[test]
fn a_saved_font_size_is_clamped_and_read_back_by_the_pane() {
    let data = tempfile::tempdir().unwrap();
    let mut app = open(&data);

    app.save_settings(draft("8")).expect("the save wrote");

    // 8 is below the scale: the file holds the size the pane said it would draw, not the typed one.
    assert_eq!(
        Settings::load(data.path()).unwrap().font_size,
        Some(10.0),
        "an under-range size is stored clamped"
    );
    assert_eq!(
        app.settings().font_size,
        "10",
        "the pane opens on the size the window draws"
    );

    // A size inside the scale is kept exactly, and an empty field means the default scale.
    app.save_settings(draft("14")).expect("the save wrote");
    assert_eq!(Settings::load(data.path()).unwrap().font_size, Some(14.0));
    assert_eq!(app.settings().font_size, "14");

    app.save_settings(draft("")).expect("the clear wrote");
    assert_eq!(
        Settings::load(data.path()).unwrap().font_size,
        None,
        "an empty field removes the override"
    );
    assert_eq!(
        app.settings().font_size,
        "",
        "the pane shows an empty field for the default scale"
    );
}
