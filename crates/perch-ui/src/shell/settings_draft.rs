//! The settings form's data: the fields a save would write and the panel's worded state.
//!
//! Split from [`settings`](super::settings) so the panel's state and the presses that open, save
//! and close it are two readable files. Nothing here writes anything.

use rune::TextEdit;

use crate::settings::{Invalid, SettingsDraft};

use super::keybind::Keybinds;
use super::model_picker::ModelPicker;
use super::settings_snapshot::{KeyStatus, ProviderView, SettingsSnapshot};

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
    /// One of the theme library names.
    pub(super) theme: String,
    /// The type scale in points, empty when the default is drawn.
    pub(super) font_size: TextEdit,
    /// The release manifest to check, empty when the app checks nothing.
    pub(super) update_check: TextEdit,
    /// The field that refused the last save, when one did.
    pub(super) error: Option<Invalid>,
    /// Whether this panel changes the session's model only, or writes the whole settings file.
    /// The composer opens the same list in this mode, so there is one picker and one set of rows.
    pub(super) session_only: bool,
    /// Whether the provider's model list is unfolded. Folded by default once a model is chosen, so
    /// the list cannot fill the pane and hold the wheel hostage above the fields below it.
    pub(super) models_open: bool,
    /// Whether a key is stored, as the panel says it after the last save.
    pub(super) key: KeyStatus,
    /// "saved", or why the app could not save: a word the person reads, never an empty panel.
    pub(super) notice: Option<String>,
    /// The provider profiles the panel lists, and the one being edited.
    pub(super) profiles: Vec<ProviderView>,
    /// Which profile the fields below belong to.
    pub(super) selected: usize,
    /// Whether the next save also makes the edited profile the active one.
    pub(super) activate: bool,
    /// The chords the window answers to, and the fields that take a new one.
    pub(super) keybinds: Keybinds,
}

impl Panel {
    /// The panel for `snapshot`, every field filled with what is saved.
    pub(super) fn new(snapshot: &SettingsSnapshot) -> Panel {
        let selected = snapshot
            .profiles
            .iter()
            .position(|profile| profile.active)
            .unwrap_or(0);
        let editing = snapshot.profiles.get(selected);
        let base_url = editing
            .map(|profile| profile.base_url.clone())
            .unwrap_or_else(|| snapshot.base_url.clone());
        let model = editing
            .map(|profile| profile.model.clone())
            .unwrap_or_else(|| snapshot.model.clone());
        let key = editing.map(|profile| profile.key).unwrap_or(snapshot.key);
        Panel {
            base_url: edit(&base_url),
            model: edit(&model),
            picker: ModelPicker::default(),
            api_key: TextEdit::default(),
            theme: snapshot.theme.clone(),
            font_size: edit(&snapshot.font_size),
            update_check: edit(&snapshot.update_check),
            error: None,
            session_only: false,
            models_open: snapshot.model.trim().is_empty(),
            key,
            notice: None,
            profiles: snapshot.profiles.clone(),
            selected,
            activate: false,
            keybinds: Keybinds::new(&snapshot.keybinds),
        }
    }

    /// The values a save would write: the fields as they stand, trimmed.
    pub fn draft(&self) -> SettingsDraft {
        SettingsDraft {
            base_url: self.base_url.text.trim().to_string(),
            model: self.model.text.trim().to_string(),
            theme: self.theme.clone(),
            font_size: self.font_size.text.trim().to_string(),
            update_check: self.update_check.text.trim().to_string(),
            api_key: self.api_key.text.trim().to_string(),
            profile: self
                .selected_profile()
                .map(|p| p.name.clone())
                .unwrap_or_default(),
            activate: self.activate,
            keybinds: self.keybinds.draft(),
        }
    }

    /// The profiles the panel lists.
    pub fn profiles(&self) -> &[ProviderView] {
        &self.profiles
    }

    /// The profile the fields below belong to.
    pub fn selected_profile(&self) -> Option<&ProviderView> {
        self.profiles.get(self.selected)
    }

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

    /// How many models the provider listed, as the picker holds them. Zero before a listing.
    pub fn listed_models(&self) -> usize {
        self.picker.rows(usize::MAX).1
    }

    /// Whether the provider's list is unfolded in the panel.
    pub fn models_open(&self) -> bool {
        self.models_open
    }

    /// Whether the provider's list is still on its way.
    pub fn models_loading(&self) -> bool {
        self.picker.loading()
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

/// A field holding `value`.
pub(super) fn edit(value: &str) -> TextEdit {
    let mut edit = TextEdit::default();
    edit.text = value.to_string();
    edit
}
