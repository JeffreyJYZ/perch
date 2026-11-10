//! The settings screen: what it refuses, what it draws, and the two ways into it.
//!
//! No keychain is involved anywhere here: the panel is opened over [`Idle`] or a test app that
//! keeps its "key" in memory, so a draft is validated, drawn and saved without a provider. The
//! key's round trip through a real store is proved in `perch-app`'s tests.

use std::time::Duration;

use perch_ui::SettingsDraft;
use perch_ui::Shell;
use perch_ui::Transcript;
use perch_ui::shell::{
    Intent, KeyStatus, Live, Panels, SettingsSnapshot, Waiting, composer_key, intent,
};
use rune::{Env, Key, KeyEvent, Mods, Runtime, Tx};

/// A draft with every field stated, so a test names only the one it is about.
fn draft(base_url: &str, model: &str, theme: &str, api_key: &str) -> SettingsDraft {
    SettingsDraft {
        base_url: base_url.to_string(),
        model: model.to_string(),
        theme: theme.to_string(),
        api_key: api_key.to_string(),
    }
}

/// The window's key press for ⌘,.
fn cmd_comma() -> KeyEvent {
    KeyEvent {
        key: Key::Command(','),
        mods: Mods {
            cmd: true,
            ..Mods::default()
        },
    }
}

/// A laid-out window over `shell`.
fn layout<L: Live + Panels>(shell: Shell<L>) -> Runtime<Shell<L>> {
    let mut runtime = Runtime::new(
        shell,
        perch_ui::shell::view,
        (1280.0, 820.0),
        Env::new(true, 1.0),
    );
    runtime.frame(0.0);
    runtime
}

#[test]
fn each_bad_field_is_named_in_words() {
    let url = draft("not a url", "test/model", "dark", "")
        .validate()
        .expect_err("a URL that does not parse is refused");
    assert_eq!(url.field, perch_ui::Field::BaseUrl);
    assert!(
        url.words.contains("base URL"),
        "the refusal names the field: {}",
        url.words
    );

    let model = draft("https://example.com/v1", "  ", "dark", "")
        .validate()
        .expect_err("an empty model is refused");
    assert_eq!(model.field, perch_ui::Field::Model);
    assert!(model.words.contains("model"), "{}", model.words);

    let theme = draft("https://example.com/v1", "test/model", "solarized", "")
        .validate()
        .expect_err("a theme the window cannot draw is refused");
    assert_eq!(theme.field, perch_ui::Field::Theme);
    assert!(theme.words.contains("theme"), "{}", theme.words);

    // An address has to be reachable over HTTP: it is a provider root, not any URL.
    let scheme = draft("ftp://example.com", "test/model", "dark", "")
        .validate()
        .expect_err("a non-HTTP address is refused");
    assert_eq!(scheme.field, perch_ui::Field::BaseUrl);

    draft("https://example.com/v1", "test/model", "dark", "")
        .validate()
        .expect("a stated draft is valid");
    draft("http://127.0.0.1:8080/v1", "test/model", "light", "")
        .validate()
        .expect("a local provider is valid");
}

#[test]
fn a_bad_field_keeps_the_panel_open_and_nothing_is_written() {
    let mut shell = Shell::idle(Vec::new(), 0);
    shell.toggle_settings();
    shell.stage_settings(draft("not a url", "test/model", "dark", ""));

    assert!(!shell.save_settings(), "nothing was saved");
    assert!(shell.settings_open(), "the panel stays open");
    let error = shell
        .settings_panel()
        .and_then(|panel| panel.error().cloned())
        .expect("the panel says which field refused");
    assert_eq!(error.field, perch_ui::Field::BaseUrl);
    assert_eq!(
        shell
            .settings_panel()
            .and_then(|panel| panel.notice())
            .map(str::to_string),
        None,
        "a refused field is an error, not a notice"
    );
}

#[test]
fn the_gear_and_cmd_comma_open_and_close_the_same_panel() {
    assert_eq!(intent(&cmd_comma(), Waiting::Nothing), Intent::Settings);

    // The key, applied by the one handler the composer and the window share.
    let mut typed = Shell::idle(Vec::new(), 0);
    assert!(
        composer_key(&mut typed, &cmd_comma(), &mut Tx::default()),
        "⌘, is consumed by the window"
    );
    assert!(typed.settings_open(), "⌘, opens the settings");

    // The gear, pressed where the window drew it.
    let mut runtime = layout(Shell::idle(Vec::new(), 0));
    let gear = runtime
        .elements()
        .into_iter()
        .find(|element| element.key.as_deref() == Some("settings-gear"))
        .expect("the tab row draws the gear");
    assert_eq!(gear.role, "button");
    assert_eq!(gear.label.as_deref(), Some("Settings"));
    let at = (
        gear.rect[0] + gear.rect[2] * 0.5,
        gear.rect[1] + gear.rect[3] * 0.5,
    );
    runtime.pointer_down(at.0, at.1);
    runtime.pointer_up(at.0, at.1);
    runtime.frame(0.0);
    assert!(runtime.state.settings_open(), "the gear opens the settings");

    assert_eq!(
        runtime.state.settings_panel().map(|panel| panel.theme()),
        typed.settings_panel().map(|panel| panel.theme()),
        "both ways reach the same panel"
    );

    // And the same press again is the same press: both close it.
    assert!(composer_key(&mut typed, &cmd_comma(), &mut Tx::default()));
    assert!(!typed.settings_open(), "⌘, closes the panel it opened");
    let gear = runtime
        .elements()
        .into_iter()
        .find(|element| element.key.as_deref() == Some("settings-gear"))
        .expect("the gear is still drawn");
    let at = (
        gear.rect[0] + gear.rect[2] * 0.5,
        gear.rect[1] + gear.rect[3] * 0.5,
    );
    runtime.pointer_down(at.0, at.1);
    runtime.pointer_up(at.0, at.1);
    runtime.frame(0.0);
    assert!(!runtime.state.settings_open(), "the gear closes it too");
}

#[test]
fn the_panel_draws_every_field_and_the_key_as_a_word() {
    let mut shell = Shell::idle(Vec::new(), 0);
    shell.toggle_settings();
    let runtime = layout(shell);
    let tree = runtime.accessibility_json().to_string();
    for word in [
        "Settings", "Base URL", "Model", "Theme", "API key", "Save", "missing",
    ] {
        assert!(tree.contains(word), "the panel is missing {word}:\n{tree}");
    }
    // The words are laid out, not merely present: the panel has real boxes to press.
    let elements = runtime.elements();
    for key in [
        "settings-base-url",
        "settings-model",
        "settings-key",
        "settings-key-state",
        "settings-save",
    ] {
        let rect = elements
            .iter()
            .find(|element| element.key.as_deref() == Some(key))
            .map(|element| element.rect)
            .unwrap_or_else(|| panic!("the panel draws no {key}"));
        assert!(
            rect[2] > 0.0 && rect[3] > 0.0,
            "{key} has no box to press: {rect:?}"
        );
    }
}

/// The app behind the window, with no keychain: a stored key is a word and a draft recorded.
struct Fake {
    key: KeyStatus,
    saved: Option<SettingsDraft>,
}

/// A fake with no plugin host has no panel column, which is the default.
impl Panels for Fake {}

impl Live for Fake {
    fn pump(&mut self, _: Duration) -> bool {
        false
    }
    fn transcript(&self) -> Option<&Transcript> {
        None
    }
    fn waiting(&self) -> Waiting {
        Waiting::Nothing
    }
    fn busy(&self) -> bool {
        false
    }
    fn prompt(&mut self, _: String) -> std::result::Result<(), String> {
        Ok(())
    }
    fn cancel(&mut self) {}
    fn approve(&mut self) {}
    fn deny(&mut self) {}
    fn pick(&mut self, _: usize) {}
    fn switch(&mut self, _: usize) {}
    fn settings(&self) -> Option<SettingsSnapshot> {
        Some(SettingsSnapshot {
            base_url: "https://example.com/v1".to_string(),
            model: "test/model".to_string(),
            theme: "dark".to_string(),
            key: self.key,
        })
    }
    fn save_settings(&mut self, draft: SettingsDraft) -> std::result::Result<KeyStatus, String> {
        self.saved = Some(draft);
        Ok(KeyStatus::Stored)
    }
}

#[test]
fn a_saved_key_is_worded_and_never_held_by_the_panel() {
    let mut shell = Shell::new(Vec::new(), 0).with_live(Fake {
        key: KeyStatus::Missing,
        saved: None,
    });
    shell.toggle_settings();
    shell.stage_settings(draft(
        "https://example.com/v1",
        "test/model",
        "dark",
        "sk-secret",
    ));

    assert!(shell.save_settings(), "the app took the draft");
    let panel = shell.settings_panel().expect("the panel stays open");
    assert_eq!(panel.key_words(), "stored", "the panel says the word");
    assert_eq!(
        panel.draft().api_key,
        "",
        "the key field is emptied: the panel never shows the secret again"
    );
    assert_eq!(panel.notice(), Some("saved"));

    let saved = shell
        .live()
        .and_then(|fake| fake.saved.clone())
        .expect("the app was handed the key");
    assert_eq!(
        saved.api_key, "sk-secret",
        "the key went to the app, not the panel"
    );

    let tree = layout(shell).accessibility_json().to_string();
    assert!(tree.contains("stored"), "the panel shows the word: {tree}");
    assert!(
        !tree.contains("sk-secret"),
        "the secret is never drawn:\n{tree}"
    );
}
