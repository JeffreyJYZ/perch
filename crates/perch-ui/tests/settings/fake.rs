//! The app behind the window, with no keychain and no provider: a stored key is a word, a draft is
//! recorded, and the model list is whatever the test put in it.
//!
//! The listing is two-step like the real one: asking starts it, and a later frame takes the answer.
//! That is what makes a test able to prove the pane opened before the provider was asked.

use std::collections::HashMap;
use std::time::Duration;

use perch_agent::ListedModel;
use perch_ui::SettingsDraft;
use perch_ui::Transcript;
use perch_ui::shell::{Bindings, KeyStatus, Live, Panels, ProviderView, SettingsSnapshot, Waiting};

pub struct Fake {
    pub key: KeyStatus,
    pub saved: Option<SettingsDraft>,
    pub model: String,
    pub models: Vec<String>,
    /// The context window each listed model states, by id.
    pub limits: HashMap<String, u32>,
    pub failure: Option<String>,
    pub picked: Option<String>,
    /// Whether a listing is "on its way": [`Live::models_ready`] answers it once.
    pub listing: bool,
    /// The last prompt sent, so a test can prove `/compact` never becomes one.
    pub prompted: Option<String>,
    /// How many compactions were asked for.
    pub compacted: u32,
    /// The last turn's tokens, for the meter.
    pub tokens: Option<u32>,
    /// The last turn's dollars, for the meter, when the provider stated a price.
    pub cost: Option<f64>,
    /// The theme the saved settings carry, which is what the window draws.
    pub theme: String,
    /// The type scale the saved settings carry, empty for the default scale.
    pub font_size: String,
    /// The shell command the saved settings carry, empty for this machine's own shell.
    pub shell: String,
    /// The provider profiles the settings carry, as the panel lists them.
    pub profiles: Vec<ProviderView>,
    /// The chords the settings carry, over the defaults.
    pub keybinds: Bindings,
}

impl Default for Fake {
    fn default() -> Fake {
        Fake {
            key: KeyStatus::Missing,
            saved: None,
            model: "test/model".to_string(),
            models: Vec::new(),
            limits: HashMap::new(),
            failure: None,
            picked: None,
            listing: false,
            prompted: None,
            compacted: 0,
            tokens: None,
            cost: None,
            theme: "dark".to_string(),
            font_size: String::new(),
            shell: String::new(),
            profiles: Vec::new(),
            keybinds: Bindings::defaults(),
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
    fn prompt(&mut self, text: String) -> std::result::Result<(), String> {
        self.prompted = Some(text);
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
    fn models(&mut self) -> std::result::Result<Vec<ListedModel>, String> {
        self.models_ready()
            .unwrap_or_else(|| panic!("the fake lists nothing until a listing is started"))
    }
    fn start_models(&mut self) {
        self.listing = true;
    }
    fn models_ready(&mut self) -> Option<std::result::Result<Vec<ListedModel>, String>> {
        if !self.listing {
            return None;
        }
        self.listing = false;
        Some(match &self.failure {
            Some(words) => Err(words.clone()),
            None => Ok(self
                .models
                .iter()
                .map(|id| ListedModel {
                    id: id.clone(),
                    context: self.limits.get(id).copied(),
                    // The fake's provider states no prices: a listing that says nothing about
                    // money is the ordinary case, and what a price draws is the meter's business.
                    prompt_price: None,
                    completion_price: None,
                })
                .collect()),
        })
    }
    fn model_limit(&self) -> Option<u32> {
        self.limits.get(&self.model).copied()
    }
    fn tokens_used(&self) -> Option<u32> {
        self.tokens
    }
    fn turn_cost(&self) -> Option<f64> {
        self.cost
    }
    fn compact(&mut self) {
        self.compacted += 1;
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
            font_size: self.font_size.clone(),
            update_check: String::new(),
            shell: self.shell.clone(),
            key: self.key,
            profiles: self.profiles.clone(),
            keybinds: self.keybinds.clone(),
        })
    }
    fn save_settings(&mut self, draft: SettingsDraft) -> std::result::Result<KeyStatus, String> {
        self.model = draft.model.clone();
        self.theme = draft.theme.clone();
        self.font_size = draft.font_size.clone();
        self.shell = draft.shell.clone();
        self.saved = Some(draft);
        Ok(KeyStatus::Stored)
    }
}
