//! The settings panel's state, and the presses that open, close and save it.
//!
//! The panel is a draft, never the live settings: nothing is written until Save validates it.

use rune::TextEdit;

use crate::settings::SettingsDraft;

use super::keybind::Bindable;
use super::live::Live;
use super::pane::Pane;
use super::settings_draft::edit;
use super::state::Shell;

pub use super::settings_draft::Panel;

impl<L: Live> Shell<L> {
    /// Whether the settings pane is drawn in place of the transcript. It answers from the pane,
    /// never from the panel being built, so the strip's active tab cannot disagree with it.
    pub fn settings_open(&self) -> bool {
        matches!(self.pane, Pane::Settings)
    }

    /// The panel, when it is open.
    pub fn settings_panel(&self) -> Option<&Panel> {
        self.settings.as_ref()
    }

    /// Shows the settings pane, filled with what is saved, or returns to the session when it is
    /// already showing: the settings tab and ⌘, take this one action, so neither can drift.
    pub fn toggle_settings(&mut self) {
        if self.settings_open() {
            self.close_settings();
            return;
        }
        let snapshot = self
            .live
            .as_ref()
            .and_then(Live::settings)
            .unwrap_or_default();
        self.set_theme(&snapshot.theme);
        self.refresh_appearance();
        self.settings = Some(Panel::new(&snapshot));
        self.pane = Pane::Settings;
        // The list is loaded as the panel opens: an empty list that waits for Refresh looks broken.
        self.refresh_models();
    }

    /// Hides the settings pane, discarding the draft, and returns to the session: nothing unsaved
    /// is written.
    pub fn close_settings(&mut self) {
        self.settings = None;
        self.back_to_session();
    }

    /// Selects the provider profile at `index`: the fields below the list become that profile's.
    ///
    /// The key field is emptied, never filled: a key is write-only from the window's side, so a
    /// stored one has no path back into a field.
    pub fn select_profile(&mut self, index: usize) {
        if let Some(panel) = self.settings.as_mut() {
            panel.select(index);
        }
    }

    /// Adds a provider profile to the list and selects it, empty, so its fields can be filled in.
    ///
    /// Nothing is written until Save: the list the panel draws is the draft, like every other
    /// field in it.
    pub fn add_profile(&mut self) {
        if let Some(panel) = self.settings.as_mut() {
            panel.add();
        }
    }

    /// Makes the selected provider profile the active one, and saves.
    ///
    /// The next turn asks it: the address, the key and the model all follow, with no restart and
    /// no session reopened. A profile whose fields are refused is not activated, and the pane
    /// says which field refused it.
    pub fn activate_profile(&mut self) -> bool {
        let Some(panel) = self.settings.as_mut() else {
            return false;
        };
        panel.activate = true;
        let saved = self.save_settings();
        if let Some(panel) = self.settings.as_mut() {
            panel.activate = false;
        }
        saved
    }

    /// Replaces the draft, as a preset or a test does. Only the fields a draft carries change.
    pub fn stage_settings(&mut self, draft: SettingsDraft) {
        let Some(panel) = self.settings.as_mut() else {
            return;
        };
        // A draft that names a profile edits that profile: the fields below the list are its.
        if !draft.profile.is_empty()
            && let Some(index) = panel
                .profiles()
                .iter()
                .position(|p| p.name == draft.profile)
        {
            panel.select(index);
        }
        panel.base_url = edit(&draft.base_url);
        panel.model = edit(&draft.model);
        panel.api_key = edit(&draft.api_key);
        panel.theme = draft.theme;
        panel.font_size = edit(&draft.font_size);
        panel.shell = edit(&draft.shell);
        panel.update_check = edit(&draft.update_check);
        // A chord the draft names is the field's text, exactly as typing it into it would be.
        for (index, intent) in Bindable::ALL.iter().enumerate() {
            let chord = draft
                .keybinds
                .get(intent.name())
                .cloned()
                .unwrap_or_default();
            if let Some(field) = panel.keybinds.field_mut(index) {
                *field = edit(&chord);
            }
        }
        panel.activate = draft.activate;
        panel.error = None;
        panel.notice = None;
    }

    /// Validates the draft and saves it. Reports whether the settings reached the app: a field
    /// that fails names itself and nothing is written, and a save the app refuses leaves the panel
    /// open, so the value is not lost with the window.
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
                // The app has just written `settings.toml`, so ask it what the file now says: the
                // theme and the type size are drawn from here, with no restart.
                self.refresh_appearance();
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

/// The presses a settings field takes before the window's own map: Escape leaves the settings and
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
