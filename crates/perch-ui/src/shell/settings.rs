//! The settings panel's state, and the presses that open, close and save it.
//!
//! The panel is a draft, never the live settings: nothing is written until Save validates it.

use rune::TextEdit;

use crate::settings::{Invalid, SettingsDraft};

use super::live::{KeyStatus, Live, SettingsSnapshot};
use super::model_picker::ModelPicker;
use super::state::Shell;

/// The settings form as the window holds it.
pub struct Panel {
    /// The provider's API root, as the field shows it.
    pub(super) base_url: TextEdit,
    /// The model every new session asks for.
    pub(super) model: TextEdit,
    /// The provider's list, its filter and why it could not be listed when it could not.
    pub(super) picker: ModelPicker,
    /// A key to store. Emptied after a save: the store holds it, the panel never does.
    pub(super) api_key: TextEdit,
    /// One of [`THEMES`].
    pub(super) theme: String,
    /// The release manifest to check, empty when the app checks nothing.
    pub(super) update_check: TextEdit,
    /// The field that refused the last save, when one did.
    pub(super) error: Option<Invalid>,
    /// Whether this panel changes the session's model only, or writes the whole settings file.
    /// The composer opens the same list in this mode, so there is one picker and one set of rows.
    pub(super) session_only: bool,
    /// Whether a key is stored, as the panel says it after the last save.
    pub(super) key: KeyStatus,
    /// "saved", or why the app could not save: a word the person reads, never an empty panel.
    pub(super) notice: Option<String>,
}

impl Panel {
    /// The panel for `snapshot`, every field filled with what is saved.
    pub(super) fn new(snapshot: &SettingsSnapshot) -> Panel {
        Panel {
            base_url: edit(&snapshot.base_url),
            model: edit(&snapshot.model),
            picker: ModelPicker::default(),
            api_key: TextEdit::default(),
            theme: snapshot.theme.clone(),
            update_check: edit(&snapshot.update_check),
            error: None,
            session_only: false,
            key: snapshot.key,
            notice: None,
        }
    }

    /// The values a save would write: the fields as they stand, trimmed.
    pub fn draft(&self) -> SettingsDraft {
        SettingsDraft {
            base_url: self.base_url.text.trim().to_string(),
            model: self.model.text.trim().to_string(),
            theme: self.theme.clone(),
            update_check: self.update_check.text.trim().to_string(),
            api_key: self.api_key.text.trim().to_string(),
        }
    }

    /// What the panel says about the key: a word, never the key.
    pub fn key_words(&self) -> &'static str {
        self.key.words()
    }

    /// The field that refused the last save, when one did.
    pub fn error(&self) -> Option<&Invalid> {
        self.error.as_ref()
    }

    /// "saved", or why the save the app refused.
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// The theme the panel shows selected.
    pub fn theme(&self) -> &str {
        &self.theme
    }
}

impl<L: Live> Shell<L> {
    /// Whether the settings panel is drawn in place of the transcript.
    pub fn settings_open(&self) -> bool {
        self.settings.is_some()
    }

    /// The panel, when it is open.
    pub fn settings_panel(&self) -> Option<&Panel> {
        self.settings.as_ref()
    }

    /// Shows the settings panel, filled with what is saved, or hides it when it is showing.
    ///
    /// The gear, ⌘, and the settings tab all take this one action: none of them knows how the
    /// panel is built, so none can drift into opening a different form.
    pub fn toggle_settings(&mut self) {
        if self.settings_open() {
            self.settings = None;
            return;
        }
        let snapshot = self
            .live
            .as_ref()
            .and_then(Live::settings)
            .unwrap_or_default();
        self.set_theme(&snapshot.theme);
        self.settings = Some(Panel::new(&snapshot));
        // The list is loaded as the panel opens: it is the picker's only content, and a panel
        // that draws an empty list until someone presses Refresh is a panel that looks broken.
        self.refresh_models();
    }

    /// Hides the panel, discarding the draft: nothing unsaved is written.
    pub fn close_settings(&mut self) {
        self.settings = None;
    }

    /// Replaces the draft, as a preset or a test does. Only the fields a draft carries change.
    pub fn stage_settings(&mut self, draft: SettingsDraft) {
        let Some(panel) = self.settings.as_mut() else {
            return;
        };
        panel.base_url = edit(&draft.base_url);
        panel.model = edit(&draft.model);
        panel.api_key = edit(&draft.api_key);
        panel.theme = draft.theme;
        panel.update_check = edit(&draft.update_check);
        panel.error = None;
        panel.notice = None;
    }

    /// Validates the draft and saves it. Reports whether the settings reached the app.
    ///
    /// A field that fails names itself in the panel and nothing is written; a save the app
    /// refuses says why and leaves the panel open, so the value is not lost with the window.
    pub fn save_settings(&mut self) -> bool {
        let Some(panel) = self.settings.as_ref() else {
            return false;
        };
        let draft = panel.draft();
        let theme = draft.theme.clone();
        if let Err(invalid) = draft.validate() {
            let panel = self.settings.as_mut().expect("the panel is open");
            panel.error = Some(invalid);
            panel.notice = None;
            return false;
        }
        let saved = match self.live.as_mut() {
            Some(live) => live.save_settings(draft),
            None => Err("no app is running, so nothing was saved".to_string()),
        };
        let panel = self.settings.as_mut().expect("the panel is open");
        match saved {
            Ok(key) => {
                panel.error = None;
                panel.key = key;
                // The key is written, so the field is emptied: only the word "stored" remains.
                panel.api_key = TextEdit::default();
                panel.notice = Some("saved".to_string());
                self.set_theme(&theme);
                true
            }
            Err(words) => {
                panel.error = None;
                panel.notice = Some(words);
                false
            }
        }
    }
}

/// A field holding `value`.
fn edit(value: &str) -> TextEdit {
    let mut edit = TextEdit::default();
    edit.text = value.to_string();
    edit
}

/// The presses a settings field takes before the window's own map: Escape closes the panel and
/// ⌘⏎ saves, so the form is usable without reaching for the mouse.
pub fn settings_key<L: Live>(shell: &mut Shell<L>, ev: &rune::KeyEvent, _: &mut rune::Tx) -> bool {
    match &ev.key {
        rune::Key::Escape if !ev.mods.cmd && !ev.mods.alt => {
            shell.close_settings();
            true
        }
        rune::Key::Enter if ev.mods.cmd => {
            shell.save_settings();
            true
        }
        _ => false,
    }
}
