//! The keybind part of the settings panel: the chords as they are, and the fields that take a
//! new one.
//!
//! A field is empty until a chord is typed into it, and an empty field means the intent keeps
//! the chord it has: the defaults are the window's own, and only what a person typed is saved.

use std::collections::BTreeMap;

use rune::TextEdit;

use super::{Bindings, KeybindView};

/// The panel's keybinds: what each intent answers to, and the fields that take a new chord.
pub struct Keybinds {
    /// What each intent answers to now, and why a saved chord could not be used.
    pub views: Vec<KeybindView>,
    /// One field per intent: a new chord, empty until one is typed.
    fields: Vec<TextEdit>,
}

impl Keybinds {
    /// The panel's keybinds, for the chords the window answers to now.
    pub fn new(bindings: &Bindings) -> Keybinds {
        let views = bindings.views();
        let fields = views.iter().map(|_| TextEdit::default()).collect();
        Keybinds { views, fields }
    }

    /// The bindings the fields name: an intent the person typed a chord for.
    ///
    /// An empty field is not in the map, so a save leaves that intent as it was and a file
    /// nobody touched stays empty — and therefore absent from `settings.toml` altogether.
    pub fn draft(&self) -> BTreeMap<String, String> {
        let mut binds = BTreeMap::new();
        for (view, field) in self.views.iter().zip(&self.fields) {
            let chord = field.text.trim();
            if !chord.is_empty() {
                binds.insert(view.name.clone(), chord.to_string());
            }
        }
        binds
    }

    /// The field of the intent at `index`, for the pane to draw.
    pub fn field(&self, index: usize) -> Option<&TextEdit> {
        self.fields.get(index)
    }

    /// The field of the intent at `index`, for the pane to edit.
    pub fn field_mut(&mut self, index: usize) -> Option<&mut TextEdit> {
        self.fields.get_mut(index)
    }
}
