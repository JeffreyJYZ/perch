//! The presses that open, fill and pick from the model list.
//!
//! Split from [`model_picker`](super::model_picker) so the picker's data and the shell methods that
//! drive it are two readable files. Nothing here fetches anything: the list arrives on its own
//! thread and a frame takes it when it is there.

use rune::TextEdit;

use super::live::Live;
use super::state::Shell;

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
    ///
    /// The panel is opened through [`Shell::toggle_settings`] rather than by assigning the draft,
    /// so whatever else selecting the settings region comes to mean — a pane, a tab — happens on
    /// this path too.
    pub fn open_model_panel(&mut self) {
        if !self.settings_open() {
            self.toggle_settings();
        }
        if let Some(panel) = self.settings.as_mut() {
            panel.session_only = true;
            // The panel exists here to choose a model, so the list is what it opens on.
            panel.models_open = true;
            panel.error = None;
            panel.notice = None;
        }
        self.refresh_models();
    }

    /// Asks the app for the provider's list and returns at once. The answer arrives through
    /// [`Shell::absorb_models`] on a later frame, so opening the panel is never a network wait.
    pub fn refresh_models(&mut self) {
        let Some(live) = self.live.as_mut() else {
            if let Some(panel) = self.settings.as_mut() {
                panel.picker.fill(Err(
                    "no app is running, so no models were listed".to_string()
                ));
            }
            return;
        };
        live.start_models();
        if let Some(panel) = self.settings.as_mut() {
            panel.picker.begin();
        }
    }

    /// Takes the listing once the thread that asked for it has an answer, and reports whether one
    /// arrived. Answers false while the provider is still being asked, so a frame can say so.
    pub fn absorb_models(&mut self) -> bool {
        let answer = match self.live.as_mut() {
            Some(live) => live.models_ready(),
            None => None,
        };
        match answer {
            Some(answer) => {
                if let Some(panel) = self.settings.as_mut() {
                    panel.picker.fill(answer);
                }
                true
            }
            None => false,
        }
    }

    /// Whether the open panel is still waiting on the provider's list.
    ///
    /// A window with a listing in flight keeps asking for frames while it is, or the answer would
    /// land on a window that never draws again and the list would stay empty until a key press.
    pub fn models_loading(&self) -> bool {
        self.settings
            .as_ref()
            .is_some_and(|panel| panel.picker.loading())
    }

    /// Unfolds or folds the provider's list in the settings form.
    ///
    /// Folded, the form is short enough that the fields under the list stay in reach; unfolded, the
    /// list scrolls inside its own height instead of pushing them off the pane.
    pub fn toggle_models(&mut self) {
        if let Some(panel) = self.settings.as_mut() {
            panel.models_open = !panel.models_open;
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
        // The one close path: a pane or a tab the window grows later is cleared here too.
        self.close_settings();
        true
    }
}

/// A field holding `value`.
fn edit(value: &str) -> TextEdit {
    let mut edit = TextEdit::default();
    edit.text = value.to_string();
    edit
}
