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

use super::live::Live;
use super::settings::Panel;
use super::state::Shell;

/// How many rows the list shows before it asks for a narrower search.
pub const SHOWN: usize = 30;

/// The picker's state: the last listing and the filter over it.
#[derive(Default)]
pub struct ModelPicker {
    /// The filter text: a case-insensitive substring of the model id.
    pub(super) query: TextEdit,
    /// The ids the provider listed, in the order the client returned them (already sorted).
    pub(super) ids: Vec<String>,
    /// Why nothing was listed, when nothing was.
    pub(super) failure: Option<String>,
}

impl ModelPicker {
    /// Fills the picker from a listing, keeping the filter the person typed.
    ///
    /// An empty list is stated as a failure too: a picker with no rows and no words is a picker
    /// that looks broken, and typing an id is still a way to work.
    pub fn fill(&mut self, listed: std::result::Result<Vec<String>, String>) {
        match listed {
            Ok(ids) if ids.is_empty() => {
                self.ids = Vec::new();
                self.failure =
                    Some("the provider listed no models: type an id instead".to_string());
            }
            Ok(ids) => {
                self.ids = ids;
                self.failure = None;
            }
            Err(words) => {
                self.ids = Vec::new();
                self.failure = Some(words);
            }
        }
    }

    /// Why nothing was listed, in words.
    pub fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    /// The filter text, as the field holds it.
    pub fn query(&self) -> &TextEdit {
        &self.query
    }

    /// The ids the filter keeps, and the whole set's count, so a capped list can say so.
    pub fn rows(&self, limit: usize) -> (Vec<String>, usize) {
        let needle = self.query.text.trim().to_lowercase();
        let matched: Vec<String> = self
            .ids
            .iter()
            .filter(|id| needle.is_empty() || id.to_lowercase().contains(&needle))
            .cloned()
            .collect();
        let total = matched.len();
        (matched.into_iter().take(limit).collect(), total)
    }
}

impl<L: Live> Shell<L> {
    /// The model the active session asks for, as the composer shows it. Empty is no choice yet.
    pub fn current_model(&self) -> String {
        self.live.as_ref().and_then(Live::model).unwrap_or_default()
    }

    /// Opens the model list in place of the transcript, for the current session only.
    ///
    /// It is the composer's Change press: the prompt column has no room for a list, so the list
    /// takes the region the transcript uses and returns there when a model is picked or the panel
    /// is closed. Nothing is written to `settings.toml` from here.
    pub fn open_model_panel(&mut self) {
        let snapshot = self
            .live
            .as_ref()
            .and_then(Live::settings)
            .unwrap_or_default();
        let mut panel = Panel::new(&snapshot);
        panel.session_only = true;
        self.settings = Some(panel);
        self.refresh_models();
    }

    /// Asks the app for the provider's list, and keeps the answer in the open panel's picker.
    pub fn refresh_models(&mut self) {
        let listed = self.listed();
        if let Some(panel) = self.settings.as_mut() {
            panel.picker.fill(listed);
        }
    }

    /// Picks `id` from the open panel.
    ///
    /// In a session's scope it changes that session's next turn and closes the panel; in the
    /// settings scope it stages the draft and saves, which is what applies it without a restart.
    pub fn pick_settings_model(&mut self, id: &str) -> bool {
        let Some(panel) = self.settings.as_mut() else {
            return false;
        };
        if !panel.session_only {
            panel.model = edit(id);
            panel.error = None;
            return self.save_settings();
        }
        let picked = match self.live.as_mut() {
            Some(live) => live.choose_model(id.to_string()),
            None => Err("no app is running, so the model was not changed".to_string()),
        };
        if let Err(words) = picked {
            if let Some(panel) = self.settings.as_mut() {
                panel.notice = Some(words);
            }
            return false;
        }
        self.settings = None;
        true
    }

    /// The app's listing, or the words for why there is none.
    fn listed(&mut self) -> std::result::Result<Vec<String>, String> {
        match self.live.as_mut() {
            Some(live) => live.models(),
            None => Err("no app is running, so no models were listed".to_string()),
        }
    }
}

/// A field holding `value`.
fn edit(value: &str) -> TextEdit {
    let mut edit = TextEdit::default();
    edit.text = value.to_string();
    edit
}
