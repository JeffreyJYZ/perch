//! The app behind the window, with no keychain and no provider: a stored key is a word, a draft is
//! recorded, and the model list is whatever the test put in it.

use std::time::Duration;

use perch_ui::SettingsDraft;
use perch_ui::Transcript;
use perch_ui::shell::{KeyStatus, Live, Panels, SettingsSnapshot, Waiting};

pub struct Fake {
    pub key: KeyStatus,
    pub saved: Option<SettingsDraft>,
    pub model: String,
    pub models: Vec<String>,
    pub failure: Option<String>,
    pub picked: Option<String>,
    /// The theme the saved settings carry, which is what the window draws.
    pub theme: String,
}

impl Default for Fake {
    fn default() -> Fake {
        Fake {
            key: KeyStatus::Missing,
            saved: None,
            model: "test/model".to_string(),
            models: Vec::new(),
            failure: None,
            picked: None,
            theme: "dark".to_string(),
        }
    }
}

/// A fake with no plugin host has no panel column, which is the default.
impl Panels for Fake {}

impl Live for Fake {
    fn pump(&mut self, _: Duration) -> bool {
        false
    }
    fn transcript(&self) -> Option<&Transcript> {
        None
    }
    fn waiting(&self) -> Waiting {
        Waiting::Nothing
    }
    fn busy(&self) -> bool {
        false
    }
    fn prompt(&mut self, _: String) -> std::result::Result<(), String> {
        Ok(())
    }
    fn cancel(&mut self) {}
    fn approve(&mut self) {}
    fn deny(&mut self) {}
    fn pick(&mut self, _: usize) {}
    fn switch(&mut self, _: usize) {}
    fn model(&self) -> Option<String> {
        Some(self.model.clone())
    }
    fn models(&mut self) -> std::result::Result<Vec<String>, String> {
        match &self.failure {
            Some(words) => Err(words.clone()),
            None => Ok(self.models.clone()),
        }
    }
    fn choose_model(&mut self, id: String) -> std::result::Result<(), String> {
        self.picked = Some(id.clone());
        self.model = id;
        Ok(())
    }
    fn settings(&self) -> Option<SettingsSnapshot> {
        Some(SettingsSnapshot {
            base_url: "https://example.com/v1".to_string(),
            model: self.model.clone(),
            theme: self.theme.clone(),
            update_check: String::new(),
            key: self.key,
        })
    }
    fn save_settings(&mut self, draft: SettingsDraft) -> std::result::Result<KeyStatus, String> {
        self.model = draft.model.clone();
        self.theme = draft.theme.clone();
        self.saved = Some(draft);
        Ok(KeyStatus::Stored)
    }
}
