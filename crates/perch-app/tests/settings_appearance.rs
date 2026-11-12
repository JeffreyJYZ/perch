//! The theme files and the type size, end to end: an open window follows `settings.toml`.
//!
//! `PERCH_HOME` is the seam the theme library documents, so every test here reads the themes of a
//! temporary home and never the ones a person actually has.

use std::sync::{Arc, OnceLock};

use perch_app::{AppState, ProviderDefaults};
use perch_store::Settings;
use perch_store::secret::MemorySecrets;
use tempfile::TempDir;

/// The home every test in this file shares, set once for the process.
///
/// A test binary is its own process, so this is the only place `PERCH_HOME` moves; the directory
/// outlives the tests because the library is read on every call, never once.
fn home() -> &'static TempDir {
    static HOME: OnceLock<TempDir> = OnceLock::new();
    HOME.get_or_init(|| {
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("PERCH_HOME", home.path()) };
        home
    })
}

/// The themes directory under that home, with a `dusk` file in it.
fn themes() -> std::path::PathBuf {
    home();
    let dir = perch_ui::theme::dir();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("dusk.toml"),
        "bg = \"#101014\"\nink = \"#f0e8d8\"\nsignal = \"#d98a2b\"\n",
    )
    .unwrap();
    dir
}

fn open(data: &TempDir, secrets: &Arc<MemorySecrets>) -> AppState {
    AppState::open_with_secrets(
        data.path(),
        ProviderDefaults::default(),
        Box::new(Arc::clone(secrets)),
    )
    .expect("a data directory with no sessions opens")
}

/// Settings written the way a person writes them by hand, or the panel will once it has the field.
fn write_settings(data: &TempDir, theme: &str, font_size: Option<f32>) -> Settings {
    let mut settings = Settings::load(data.path()).unwrap();
    settings.theme = theme.to_string();
    settings.font_size = font_size;
    settings.save(data.path()).unwrap();
    settings
}

#[test]
fn a_theme_file_reaches_the_window_already_open() {
    let dir = themes();
    assert!(dir.ends_with("themes"), "{dir:?}");

    let data = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecrets::new());
    let app = open(&data, &secrets);
    assert_eq!(app.appearance().theme, "dark", "the window opens dark");

    write_settings(&data, "dusk", None);
    let appearance = app.appearance();
    assert_eq!(appearance.theme, "dusk", "no restart, the same open app");
    assert_eq!(appearance.anchors.bg, rune::color::hex("#101014").unwrap());
    assert!(appearance.note.is_none());
}

#[test]
fn an_unknown_saved_theme_draws_dark_and_the_pane_says_so() {
    themes();
    let data = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecrets::new());
    let app = open(&data, &secrets);

    write_settings(&data, "duskish", None);
    let appearance = app.appearance();
    assert_eq!(appearance.theme, "dark");
    let note = appearance
        .note
        .expect("the pane says the saved name was not used");
    assert!(note.contains("duskish"), "{note}");
    assert!(note.contains("drawing dark"), "{note}");
}

#[test]
fn a_saved_font_size_scales_the_windows_type_and_one_outside_is_clamped() {
    themes();
    let data = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecrets::new());
    let app = open(&data, &secrets);
    assert!(app.appearance().note.is_none());
    assert_eq!(
        app.appearance().kit().ty.body.size,
        13.0,
        "today's body size"
    );

    write_settings(&data, "dark", Some(14.0));
    let appearance = app.appearance();
    assert_eq!(appearance.font_size, Some(14.0));
    assert!(
        (appearance.kit().ty.body.size - 14.0).abs() < 1e-4,
        "the open window draws at the size that was saved"
    );

    write_settings(&data, "dark", Some(8.0));
    let appearance = app.appearance();
    assert_eq!(
        appearance.font_size,
        Some(10.0),
        "the stored value is clamped too"
    );
    let note = appearance
        .note
        .as_deref()
        .expect("the pane says it was clamped");
    assert!(note.contains("drawn at 10"), "{note}");
    assert!((appearance.kit().ty.body.size - 10.0).abs() < 1e-4);
}
