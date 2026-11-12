//! The chords: the defaults the window has always used, and what a saved binding replaces.

use std::collections::BTreeMap;

use rune::{Key, KeyEvent, Mods};

use super::{Bindable, Bindings};
use crate::shell::Intent;

/// A press of `key` with the command key held.
fn chord(key: char) -> KeyEvent {
    KeyEvent {
        key: Key::Command(key),
        mods: Mods {
            cmd: true,
            ..Mods::default()
        },
    }
}

/// The saved bindings of a file that holds `pairs`.
fn saved(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(name, chord)| (name.to_string(), chord.to_string()))
        .collect()
}

#[test]
fn the_defaults_are_the_chords_the_window_has_always_used() {
    let bindings = Bindings::defaults();

    assert_eq!(bindings.chord(Bindable::Settings), "mod+,");
    assert_eq!(bindings.chord(Bindable::Palette), "mod+k");
    assert_eq!(bindings.chord(Bindable::Finder), "mod+p");
    assert_eq!(bindings.chord(Bindable::Find), "mod+f");
    assert_eq!(bindings.chord(Bindable::AllowForSession), "mod+enter");
    assert!(bindings.refusal_words().is_empty(), "no default is refused");
}

#[test]
fn a_press_of_a_default_chord_asks_for_its_intent() {
    let bindings = Bindings::defaults();

    assert_eq!(bindings.resolve(&chord(',')), Some(Intent::Settings));
    assert_eq!(bindings.resolve(&chord('k')), Some(Intent::Palette));
    assert_eq!(bindings.resolve(&chord('p')), Some(Intent::Finder));
    assert_eq!(bindings.resolve(&chord('f')), Some(Intent::Find));
}

#[test]
fn a_saved_binding_replaces_the_default_and_kills_the_old_chord() {
    let bindings = Bindings::resolved(&saved(&[("settings", "mod+j")]));

    assert_eq!(
        bindings.resolve(&chord('j')),
        Some(Intent::Settings),
        "the saved chord asks for the intent"
    );
    assert_eq!(
        bindings.resolve(&chord(',')),
        Some(Intent::Ignore),
        "the chord it replaced asks for nothing"
    );
    assert_eq!(
        bindings.resolve(&chord('k')),
        Some(Intent::Palette),
        "the other intents keep their own chords"
    );
}

#[test]
fn a_chord_that_cannot_be_read_is_refused_in_words() {
    let bindings = Bindings::resolved(&saved(&[("settings", "hyper+k")]));

    let words = bindings.refusal("settings").expect("the chord is refused");
    assert!(words.contains("hyper+k"), "{words}");
    assert!(
        words.contains("not a modifier"),
        "the refusal says what is wrong: {words}"
    );
    assert_eq!(
        bindings.resolve(&chord(',')),
        Some(Intent::Settings),
        "and the intent keeps the chord it had"
    );
}

#[test]
fn a_chord_two_intents_want_is_refused_with_both_names() {
    let bindings = Bindings::resolved(&saved(&[("settings", "mod+k")]));

    let words = bindings.refusal("settings").expect("the chord is refused");
    assert!(words.contains("mod+k"), "{words}");
    assert!(words.contains("settings"), "{words}");
    assert!(words.contains("palette"), "{words}");
}

#[test]
fn an_intent_the_window_does_not_know_is_refused_and_kept() {
    let bindings = Bindings::resolved(&saved(&[("nonsense", "mod+j")]));

    let words = bindings.refusal("nonsense").expect("the name is refused");
    assert!(words.contains("nonsense"), "{words}");
}
