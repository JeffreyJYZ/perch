//! The panel's profile list: which profile the fields below are editing, and one added by hand.
//!
//! Split from [`settings_draft`](super::settings_draft) so the form's fields and the panel's own
//! file stay under the 200-line budget. Nothing here writes anything.

use rune::TextEdit;

use super::settings_draft::{Panel, edit};
use super::settings_snapshot::{KeyStatus, ProviderView};

impl Panel {
    /// Selects the profile at `index` and fills the fields with it.
    ///
    /// The key field is emptied, never filled: a key is write-only from the window's side, so a
    /// stored one has no path back into a field.
    pub(super) fn select(&mut self, index: usize) {
        let Some(profile) = self.profiles.get(index) else {
            return;
        };
        self.selected = index;
        self.base_url = edit(&profile.base_url);
        self.model = edit(&profile.model);
        self.api_key = TextEdit::default();
        self.key = profile.key;
        self.error = None;
        self.notice = None;
    }

    /// Adds a profile to the list and selects it, empty, so its fields can be filled in.
    pub(super) fn add(&mut self) {
        let name = self.fresh_name();
        self.profiles.push(ProviderView {
            name,
            base_url: String::new(),
            model: String::new(),
            key: KeyStatus::Missing,
            active: false,
        });
        self.select(self.profiles.len() - 1);
    }

    /// A name no profile in the list carries.
    fn fresh_name(&self) -> String {
        let mut index = self.profiles.len() + 1;
        loop {
            let name = format!("provider-{index}");
            if !self.profiles.iter().any(|profile| profile.name == name) {
                return name;
            }
            index += 1;
        }
    }
}
