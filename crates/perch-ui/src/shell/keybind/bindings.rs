//! The chords the window answers to, and what a saved binding replaces.
//!
//! The defaults are the ones the keymap has always had; a saved binding takes one over, and the
//! chord it replaced answers nothing.

use std::collections::BTreeMap;

use perch_store::settings::Chord;
use rune::{Key, KeyEvent};

use super::super::keymap::{Intent, intent};
use super::super::live::Live;
use super::super::state::Shell;
use super::super::waiting::Waiting;
use super::{Bindable, KeybindView};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bindings {
    /// The chord each intent answers to now.
    chords: Vec<(Bindable, Chord)>,
    /// The chords the window used before anybody rebound one.
    defaults: Vec<(Bindable, Chord)>,
    /// Why a saved chord could not be used: the intent's name and the words.
    refusals: Vec<(String, String)>,
}

impl Default for Bindings {
    fn default() -> Self {
        Self::defaults()
    }
}

impl Bindings {
    /// The chords the window has always used.
    pub fn defaults() -> Self {
        let chords: Vec<(Bindable, Chord)> = Bindable::ALL
            .iter()
            .map(|bindable| {
                (
                    *bindable,
                    Chord::parse(bindable.default_chord()).expect("a default chord parses"),
                )
            })
            .collect();
        Self {
            chords: chords.clone(),
            defaults: chords,
            refusals: Vec::new(),
        }
    }

    /// The saved bindings over the defaults, and the refusals about the ones that cannot be used.
    ///
    /// A chord that cannot be read, or that two intents want, is refused in words and never
    /// dropped: the intent keeps its default and the person is told why.
    pub fn resolved(saved: &BTreeMap<String, String>) -> Self {
        let mut bindings = Self::defaults();
        for (name, text) in saved {
            let Some(bindable) = Bindable::ALL.iter().find(|b| b.name() == name) else {
                bindings.refusals.push((
                    name.clone(),
                    format!("{name} is not an intent this window knows"),
                ));
                continue;
            };
            let chord = match Chord::parse(text) {
                Ok(chord) => chord,
                Err(words) => {
                    bindings
                        .refusals
                        .push((name.clone(), format!("{text} is not a chord: {words}")));
                    continue;
                }
            };
            let wanted = bindings
                .chords
                .iter()
                .find(|(held, held_chord)| *held != *bindable && held_chord == &chord)
                .map(|(held, _)| *held);
            if let Some(held) = wanted {
                bindings.refusals.push((
                    name.clone(),
                    format!(
                        "{text} is bound to {name} and to {}, and one key answers one intent",
                        held.name()
                    ),
                ));
                continue;
            }
            if let Some(slot) = bindings.chords.iter_mut().find(|(b, _)| *b == *bindable) {
                slot.1 = chord;
            }
        }
        bindings
    }

    /// The chord an intent answers to, as it is written.
    pub fn chord(&self, bindable: Bindable) -> String {
        self.held(bindable).write()
    }

    /// Why a saved chord could not be used, when one could not.
    pub fn refusal(&self, name: &str) -> Option<String> {
        self.refusals
            .iter()
            .find(|(held, _)| held == name)
            .map(|(_, words)| words.clone())
    }

    /// Every refusal, as the pane shows them.
    pub fn refusal_words(&self) -> Vec<String> {
        self.refusals
            .iter()
            .map(|(_, words)| words.clone())
            .collect()
    }

    /// What the pane shows for each intent: the chord it answers to, and any refusal.
    pub fn views(&self) -> Vec<KeybindView> {
        Bindable::ALL
            .iter()
            .map(|bindable| KeybindView {
                name: bindable.name().to_string(),
                label: bindable.label().to_string(),
                chord: self.chord(*bindable),
                refusal: self.refusal(bindable.name()),
            })
            .collect()
    }

    /// What a press asks for, when it asks for one of the chords this window answers to.
    ///
    /// A chord an intent was rebound away from asks for nothing: the old chord is dead, so a
    /// person who moves the settings to ⌘J does not also get them on ⌘,.
    pub fn resolve(&self, ev: &KeyEvent) -> Option<Intent> {
        let chord = chord_of(ev)?;
        for (bindable, held) in &self.defaults {
            if *held == chord && self.held(*bindable) != *held {
                return Some(Intent::Ignore);
            }
        }
        self.chords
            .iter()
            .find(|(_, held)| *held == chord)
            .map(|(bindable, _)| bindable.intent())
    }

    /// The chord an intent holds.
    fn held(&self, bindable: Bindable) -> Chord {
        self.chords
            .iter()
            .find(|(held, _)| *held == bindable)
            .map(|(_, chord)| chord.clone())
            .unwrap_or_default()
    }
}

impl<L: Live> Shell<L> {
    /// What a press asks for, with the person's own bindings in force.
    ///
    /// The default chords are the ones the keymap knows; a saved binding replaces one of them,
    /// and the chord it replaced asks for nothing.
    pub fn intent(&self, ev: &KeyEvent) -> Intent {
        match self.bindings.resolve(ev) {
            Some(Intent::AllowForSession) if self.waiting != Waiting::Approval => Intent::Ignore,
            Some(found) => found,
            None => intent(ev, self.waiting),
        }
    }
}

/// The chord a press carries, when it carries one this grammar can name.
fn chord_of(ev: &KeyEvent) -> Option<Chord> {
    Some(Chord {
        mod_key: ev.mods.cmd,
        ctrl: ev.mods.ctrl,
        alt: ev.mods.alt,
        shift: ev.mods.shift,
        key: key_name(&ev.key)?,
    })
}

/// The name the grammar gives a key, when it has one.
fn key_name(key: &Key) -> Option<String> {
    match key {
        Key::Enter => Some("enter".to_string()),
        Key::Escape => Some("escape".to_string()),
        Key::Tab => Some("tab".to_string()),
        Key::Up => Some("up".to_string()),
        Key::Down => Some("down".to_string()),
        Key::Command(character) => Some(character.to_string()),
        Key::Text(text) if text == " " => Some("space".to_string()),
        Key::Text(text) if text.chars().count() == 1 => Some(text.to_lowercase()),
        _ => None,
    }
}
