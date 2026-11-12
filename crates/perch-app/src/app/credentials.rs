//! The provider profiles as the app switches between them: one is active, and the next turn asks
//! it without the app being restarted.

use perch_store::Settings;
use perch_store::settings::ProviderProfile;

use crate::{AppState, Error};

impl AppState {
    /// The provider profiles the settings file holds, as they are saved.
    pub fn profiles(&self) -> Vec<ProviderProfile> {
        Settings::load(&self.dir)
            .map(|settings| settings.providers)
            .unwrap_or_default()
    }

    /// The profile a new turn asks, as it is saved.
    pub fn active_profile(&self) -> Option<ProviderProfile> {
        Settings::load(&self.dir)
            .ok()
            .and_then(|settings| settings.active_profile().cloned())
    }

    /// Makes the profile named `name` the active one.
    ///
    /// The next turn asks it: the address, the key and the model all follow, with no restart and
    /// no session reopened. A name the list does not carry is refused in words.
    pub fn activate_profile(&mut self, name: &str) -> crate::Result<()> {
        let name = name.trim();
        let mut settings = Settings::load(&self.dir)?;
        let Some(profile) = settings.profile(name).cloned() else {
            return Err(Error::Model(format!("no provider is named {name}")));
        };
        settings.active_provider = profile.name.clone();
        settings.save(&self.dir)?;
        self.defaults.base_url = profile.base_url.clone();
        self.defaults.model = profile.model.clone();
        self.defaults.api_key =
            crate::settings::runtime_key(&*self.secrets, &profile.name).unwrap_or_default();
        self.follow_saved_endpoint();
        self.follow_saved_model();
        Ok(())
    }
}
