//! The model picker: the provider's list, a filter over it, and the presses that pick a model.
//!
//! The settings panel and the composer draw this one list, so "the model" means the same thing in
//! both places: whatever the provider returned, sorted, with the chosen one marked. A list that
//! cannot be fetched is stated in words, and the settings form keeps its typed field as the
//! fallback — a provider without the endpoint still works.
//!
//! The composer's control opens the same panel in a session scope: picking there changes this
//! session's next turn and leaves the saved default alone.

use rune::TextEdit;

/// How many rows the list shows before it asks for a narrower search.
pub const SHOWN: usize = 30;

/// The picker's state: the last listing and the filter over it.
#[derive(Default)]
pub struct ModelPicker {
    /// The filter text: a case-insensitive substring of the model id.
    pub(super) query: TextEdit,
    /// The models the provider listed, in the order the client returned them (already sorted),
    /// each with the context window it stated. The picker keeps the limits as well as the ids so
    /// the meter never has to ask the provider for a number it already sent.
    pub(super) models: Vec<perch_agent::ListedModel>,
    /// Why nothing was listed, when nothing was.
    pub(super) failure: Option<String>,
    /// Whether a listing is on its way: the panel says so instead of looking empty, and the click
    /// that opened it returns at once rather than waiting on a network round trip.
    pub(super) loading: bool,
}

impl ModelPicker {
    /// Fills the picker from a listing, keeping the filter the person typed.
    ///
    /// An empty list is stated as a failure too: a picker with no rows and no words is a picker
    /// that looks broken, and typing an id is still a way to work.
    pub fn fill(&mut self, listed: std::result::Result<Vec<perch_agent::ListedModel>, String>) {
        self.loading = false;
        match listed {
            Ok(models) if models.is_empty() => {
                self.models = Vec::new();
                self.failure =
                    Some("the provider listed no models: type an id instead".to_string());
            }
            Ok(models) => {
                self.models = models;
                self.failure = None;
            }
            Err(words) => {
                self.models = Vec::new();
                self.failure = Some(words);
            }
        }
    }

    /// Why nothing was listed, in words.
    pub fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    /// Says a listing is on its way, so the panel words it instead of drawing nothing.
    pub fn begin(&mut self) {
        self.loading = true;
        self.failure = None;
    }

    /// Whether a listing is still on its way.
    pub fn loading(&self) -> bool {
        self.loading
    }

    /// The filter text, as the field holds it.
    pub fn query(&self) -> &TextEdit {
        &self.query
    }

    /// The ids the filter keeps, and the whole set's count, so a capped list can say so.
    pub fn rows(&self, limit: usize) -> (Vec<String>, usize) {
        let needle = self.query.text.trim().to_lowercase();
        let matched: Vec<String> = self
            .models
            .iter()
            .map(|model| model.id.clone())
            .filter(|id| needle.is_empty() || id.to_lowercase().contains(&needle))
            .collect();
        let total = matched.len();
        (matched.into_iter().take(limit).collect(), total)
    }
}
