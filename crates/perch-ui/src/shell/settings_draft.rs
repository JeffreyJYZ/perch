//! The settings form's data: the fields a save would write and the panel's worded state.
//!
//! Split from [`settings`](super::settings) so the panel's state and the presses that open, save
//! and close it are two readable files. Nothing here writes anything.

use rune::TextEdit;

use crate::settings::{Invalid, SettingsDraft};

use super::live::{KeyStatus, SettingsSnapshot};
use super::model_picker::ModelPicker;

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
    /// One of [`THEMES`](crate::settings::THEMES).
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

/// A field holding `value`.
pub(super) fn edit(value: &str) -> TextEdit {
    let mut edit = TextEdit::default();
    edit.text = value.to_string();
    edit
}
