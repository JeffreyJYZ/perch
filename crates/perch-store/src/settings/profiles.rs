//! The provider profiles: a named list, of which one is active.
//!
//! A profile says where a provider lives and which model a turn asks it for. Its key is not here:
//! the key is the secret store's business, filed under the profile's name, so a file a person can
//! read holds no secret at all.

use serde::{Deserialize, Serialize};

use super::Settings;

/// The name the profile a file written before the list existed becomes.
pub const DEFAULT_PROFILE: &str = "default";

/// One provider: what it is called, where it lives, and which model a turn asks it for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderProfile {
    /// The name a person gave it, and the account its key is filed under.
    pub name: String,
    /// The provider's API root, such as `https://openrouter.ai/api/v1`.
    pub base_url: String,
    /// The model id a turn asks for.
    pub model: String,
}

impl Default for ProviderProfile {
    fn default() -> Self {
        Self {
            name: DEFAULT_PROFILE.to_string(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            model: String::new(),
        }
    }
}

impl Settings {
    /// The profile a new turn asks: the active one, or the first when the name names nothing.
    pub fn active_profile(&self) -> Option<&ProviderProfile> {
        self.profile(&self.active_provider)
            .or_else(|| self.providers.first())
    }

    /// The profile named `name`, when the list holds one.
    pub fn profile(&self, name: &str) -> Option<&ProviderProfile> {
        self.providers.iter().find(|profile| profile.name == name)
    }

    /// The profile named `name`, ready to change.
    pub fn profile_mut(&mut self, name: &str) -> Option<&mut ProviderProfile> {
        self.providers
            .iter_mut()
            .find(|profile| profile.name == name)
    }

    /// The active profile's address, empty when the list is empty.
    pub fn active_base_url(&self) -> String {
        self.active_profile()
            .map(|profile| profile.base_url.clone())
            .unwrap_or_default()
    }

    /// The active profile's model, empty when the list is empty.
    pub fn active_model(&self) -> String {
        self.active_profile()
            .map(|profile| profile.model.clone())
            .unwrap_or_default()
    }
}

/// Turns a file written before the provider list existed into a one-entry list.
///
/// The old file carried one address and one model at the top level. They become a profile named
/// [`DEFAULT_PROFILE`], and that profile is the active one: the machine asks the same provider
/// for the same model as before, and only the shape of the file changes.
///
/// Answers whether the file was one of those, so the caller can rewrite it in the new shape.
pub fn migrate_legacy(table: &toml::Table, settings: &mut Settings) -> bool {
    if table.contains_key("providers") {
        return false;
    }
    let Some(base_url) = table.get("provider_base_url").and_then(toml::Value::as_str) else {
        return false;
    };
    let model = table
        .get("model")
        .and_then(toml::Value::as_str)
        .unwrap_or_default();
    settings.providers = vec![ProviderProfile {
        name: DEFAULT_PROFILE.to_string(),
        base_url: base_url.to_string(),
        model: model.to_string(),
    }];
    settings.active_provider = DEFAULT_PROFILE.to_string();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::path_in;

    /// A file written before the provider list existed.
    fn old_file() -> String {
        "provider_base_url = \"https://old.test/v1\"\nmodel = \"old/model\"\ntheme = \"light\"\n"
            .to_string()
    }

    #[test]
    fn a_file_written_before_the_list_existed_becomes_one_entry() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(path_in(dir.path()), old_file()).unwrap();

        let settings = Settings::load(dir.path()).unwrap();

        assert_eq!(settings.providers.len(), 1, "the old provider is one entry");
        assert_eq!(settings.providers[0].name, DEFAULT_PROFILE);
        assert_eq!(settings.providers[0].base_url, "https://old.test/v1");
        assert_eq!(settings.providers[0].model, "old/model");
        assert_eq!(
            settings.active_provider, DEFAULT_PROFILE,
            "the profile it became is the active one"
        );
        assert_eq!(settings.theme, "light", "the rest of the file is untouched");
    }

    #[test]
    fn the_migrated_file_is_rewritten_in_the_new_shape() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(path_in(dir.path()), old_file()).unwrap();

        Settings::load(dir.path()).unwrap();

        let text = std::fs::read_to_string(path_in(dir.path())).unwrap();
        assert!(text.contains("[[providers]]"), "{text}");
        assert!(
            !text.contains("provider_base_url"),
            "the old shape is gone once it has been read:\n{text}"
        );
        let again = Settings::load(dir.path()).unwrap();
        assert_eq!(again.providers.len(), 1, "and it reads back the same");
        assert_eq!(again.providers[0].base_url, "https://old.test/v1");
    }

    #[test]
    fn a_file_that_already_has_a_list_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = Settings::default();
        settings.providers.push(ProviderProfile {
            name: "second".to_string(),
            base_url: "https://second.test/v1".to_string(),
            model: "second/model".to_string(),
        });
        settings.active_provider = "second".to_string();
        settings.save(dir.path()).unwrap();

        let loaded = Settings::load(dir.path()).unwrap();

        assert_eq!(loaded.providers.len(), 2);
        assert_eq!(loaded.active_provider, "second");
    }

    #[test]
    fn the_active_profile_is_the_one_named_and_the_first_when_the_name_names_nothing() {
        let mut settings = Settings::default();
        settings.providers.push(ProviderProfile {
            name: "second".to_string(),
            base_url: "https://second.test/v1".to_string(),
            model: "second/model".to_string(),
        });
        settings.active_provider = "second".to_string();
        assert_eq!(settings.active_base_url(), "https://second.test/v1");
        assert_eq!(settings.active_model(), "second/model");

        settings.active_provider = "gone".to_string();
        assert_eq!(
            settings
                .active_profile()
                .map(|profile| profile.name.as_str()),
            Some(DEFAULT_PROFILE),
            "a name that names nothing falls back to the first profile"
        );
    }
}
