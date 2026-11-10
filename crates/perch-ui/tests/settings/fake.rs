//! The app behind the window, with no keychain: a stored key is a word and a draft recorded.

use std::time::Duration;

use perch_ui::SettingsDraft;
use perch_ui::Transcript;
use perch_ui::shell::{KeyStatus, Live, Panels, SettingsSnapshot, Waiting};

pub struct Fake {
    pub key: KeyStatus,
    pub saved: Option<SettingsDraft>,
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
    fn settings(&self) -> Option<SettingsSnapshot> {
        Some(SettingsSnapshot {
            base_url: "https://example.com/v1".to_string(),
            model: "test/model".to_string(),
            theme: "dark".to_string(),
            key: self.key,
        })
    }
    fn save_settings(&mut self, draft: SettingsDraft) -> std::result::Result<KeyStatus, String> {
        self.saved = Some(draft);
        Ok(KeyStatus::Stored)
    }
}
