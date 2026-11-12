//! The keybinds section of the settings panel, and what a saved chord does to a press.
//!
//! No app is involved beyond the fake that carries the saved chords: a press in, an intent out,
//! exactly as the window itself asks.

use std::collections::BTreeMap;

use perch_ui::Shell;
use perch_ui::shell::{Bindings, Intent};
use rune::{Key, KeyEvent, Mods};

use crate::fake::Fake;
use crate::harness::{draft, layout};

/// A press of `key` with the command key held.
fn cmd(key: char) -> KeyEvent {
    KeyEvent {
        key: Key::Command(key),
        mods: Mods {
            cmd: true,
            ..Mods::default()
        },
    }
}

/// A shell over a fake whose settings carry `saved` chords.
fn shell_with(saved: &[(&str, &str)]) -> Shell<Fake> {
    let binds: BTreeMap<String, String> = saved
        .iter()
        .map(|(intent, chord)| (intent.to_string(), chord.to_string()))
        .collect();
    Shell::new(Vec::new(), 0).with_live(Fake {
        keybinds: Bindings::resolved(&binds),
        ..Fake::default()
    })
}

#[test]
fn a_machine_that_saves_no_chord_answers_the_defaults() {
    let shell = shell_with(&[]);

    assert_eq!(shell.intent(&cmd(',')), Intent::Settings);
    assert_eq!(shell.intent(&cmd('k')), Intent::Palette);
    assert_eq!(shell.intent(&cmd('p')), Intent::Finder);
    assert_eq!(shell.intent(&cmd('f')), Intent::Find);
    assert_eq!(
        shell.intent(&cmd('j')),
        Intent::Ignore,
        "a chord nobody bound asks for nothing"
    );
}

#[test]
fn a_saved_chord_answers_at_once_and_the_old_one_answers_nothing() {
    let shell = shell_with(&[("settings", "mod+j")]);

    assert_eq!(
        shell.intent(&cmd('j')),
        Intent::Settings,
        "the saved chord asks for the intent on the next press"
    );
    assert_eq!(
        shell.intent(&cmd(',')),
        Intent::Ignore,
        "the chord it replaced is dead"
    );
    assert_eq!(
        shell.intent(&cmd('k')),
        Intent::Palette,
        "and no other intent moved"
    );
}

#[test]
fn the_pane_lists_every_intent_with_the_chord_it_answers_to() {
    let mut shell = Shell::new(Vec::new(), 0).with_live(Fake::default());
    shell.toggle_settings();
    let tree = layout(shell).accessibility_json().to_string();

    assert!(tree.contains("KEYBINDS"), "the section is named:\n{tree}");
    for (label, chord) in [
        ("Settings", "mod+,"),
        ("Command palette", "mod+k"),
        ("File finder", "mod+p"),
        ("Search transcript", "mod+f"),
        ("Allow for session", "mod+enter"),
    ] {
        assert!(tree.contains(label), "the pane is missing {label}:\n{tree}");
        assert!(
            tree.contains(chord),
            "the pane does not say {label} answers {chord}:\n{tree}"
        );
    }
}

#[test]
fn a_chord_that_cannot_be_read_is_said_in_the_pane() {
    let mut shell = shell_with(&[("settings", "hyper+k")]);
    shell.toggle_settings();
    let tree = layout(shell).accessibility_json().to_string();

    assert!(
        tree.contains("hyper+k"),
        "the pane names the chord it could not use:\n{tree}"
    );
    assert!(
        tree.contains("not a modifier"),
        "and says what is wrong with it:\n{tree}"
    );
}

#[test]
fn a_chord_typed_in_the_pane_reaches_the_app() {
    let mut shell = Shell::new(Vec::new(), 0).with_live(Fake::default());
    shell.toggle_settings();
    let mut staged = draft("https://example.com/v1", "test/model", "dark", "");
    staged
        .keybinds
        .insert("settings".to_string(), "mod+j".to_string());
    shell.stage_settings(staged);

    assert!(shell.save_settings(), "the app took the draft");
    let saved = shell
        .live()
        .and_then(|fake| fake.saved.clone())
        .expect("the draft reached the app");
    assert_eq!(
        saved.keybinds.get("settings").map(String::as_str),
        Some("mod+j"),
        "the chord the person typed is the one saved"
    );
}

#[test]
fn a_chord_two_intents_want_is_refused_naming_both() {
    let mut shell = Shell::new(Vec::new(), 0).with_live(Fake::default());
    shell.toggle_settings();
    let mut staged = draft("https://example.com/v1", "test/model", "dark", "");
    staged
        .keybinds
        .insert("settings".to_string(), "mod+k".to_string());
    staged
        .keybinds
        .insert("palette".to_string(), "mod+k".to_string());
    shell.stage_settings(staged);

    assert!(
        !shell.save_settings(),
        "two intents on one chord is refused"
    );
    let words = shell
        .settings_panel()
        .and_then(|panel| panel.error())
        .map(|invalid| invalid.words.clone())
        .unwrap_or_default();
    assert!(
        words.contains("mod+k"),
        "the refusal names the chord: {words}"
    );
    assert!(words.contains("settings"), "and one intent: {words}");
    assert!(words.contains("palette"), "and the other: {words}");
    assert!(
        shell.live().and_then(|fake| fake.saved.clone()).is_none(),
        "a refused draft never reaches the app"
    );
}
