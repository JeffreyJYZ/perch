//! The settings a person edits: the provider's address and model, the theme, and the API key.
//!
//! [`SettingsDraft`] is the form's data and [`SettingsDraft::validate`] is the one rule about what
//! may be saved, so a bad field is refused before anything is written. The key is deliberately not
//! a field of [`Settings`](perch_store::Settings): it goes to the secret store, which is why a
//! draft can be written to `settings.toml` and never carry a secret with it.

use url::Url;

/// The theme a fresh install uses, and the one the panel offers when nothing is saved yet.
pub const DEFAULT_THEME: &str = "dark";

/// The themes the settings screen offers.
///
/// One of these and nothing else: a theme the window cannot draw would save cleanly and then show
/// as the default, which reads as a choice the app silently dropped.
pub const THEMES: [&str; 2] = ["dark", "light"];

/// Which field a refusal is about, so the panel can name it and mark the right box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    /// The provider's API root.
    BaseUrl,
    /// The model every new session asks for.
    Model,
    /// One of [`THEMES`].
    Theme,
    /// The release manifest the app checks, when one is named.
    UpdateCheck,
}

impl Field {
    /// The field's name as a person reads it, and the word a refusal starts with.
    pub fn words(self) -> &'static str {
        match self {
            Field::BaseUrl => "base URL",
            Field::Model => "model",
            Field::Theme => "theme",
            Field::UpdateCheck => "update check URL",
        }
    }
}

/// Why a draft was not saved, and the field it is about.
///
/// The words always start with the field's name, so the panel can show one line that says both
/// what is wrong and where, without a second lookup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invalid {
    /// The field the refusal is about.
    pub field: Field,
    /// The sentence the panel shows, starting with the field's name.
    pub words: String,
}

impl Invalid {
    fn new(field: Field, why: impl AsRef<str>) -> Self {
        Self {
            field,
            words: format!("{}: {}", field.words(), why.as_ref()),
        }
    }
}

/// What the settings panel holds, as the values a save would write.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SettingsDraft {
    /// The provider's API root, such as `https://openrouter.ai/api/v1`.
    pub base_url: String,
    /// The model id every new session asks for.
    pub model: String,
    /// One of [`THEMES`].
    pub theme: String,
    /// The release manifest to check, empty when the app must not reach the network.
    pub update_check: String,
    /// A key to store, empty when the person typed none.
    pub api_key: String,
}

impl SettingsDraft {
    /// Checks every field, in the order the panel shows them.
    ///
    /// An address has to parse as a URL with an HTTP scheme and a host, because the provider
    /// client appends `/chat/completions` to it: anything else would save and then fail every
    /// turn. A model has to be named, because an empty one is not a model the server knows. The
    /// theme has to be one of [`THEMES`], because anything else cannot be drawn. The update check
    /// is allowed to be empty, which is the app never reaching the network, and otherwise is held
    /// to the same URL rule as the provider's address.
    pub fn validate(&self) -> Result<(), Invalid> {
        let address = self.base_url.trim();
        let Ok(url) = Url::parse(address) else {
            return Err(Invalid::new(
                Field::BaseUrl,
                "this is not a URL, for example https://openrouter.ai/api/v1",
            ));
        };
        if !matches!(url.scheme(), "http" | "https") {
            return Err(Invalid::new(
                Field::BaseUrl,
                format!("a {} address is not reachable over HTTP", url.scheme()),
            ));
        }
        if url.host_str().is_none_or(str::is_empty) {
            return Err(Invalid::new(Field::BaseUrl, "it names no host to reach"));
        }
        if self.model.trim().is_empty() {
            return Err(Invalid::new(
                Field::Model,
                "choose a model, for example deepseek/deepseek-v4.1-flash",
            ));
        }
        if !THEMES.contains(&self.theme.as_str()) {
            return Err(Invalid::new(
                Field::Theme,
                format!("must be one of {}", THEMES.join(", ")),
            ));
        }
        let manifest = self.update_check.trim();
        if !manifest.is_empty() {
            let Ok(url) = Url::parse(manifest) else {
                return Err(Invalid::new(
                    Field::UpdateCheck,
                    "this is not a URL, for example https://release.example.com/latest.json",
                ));
            };
            if !matches!(url.scheme(), "http" | "https") {
                return Err(Invalid::new(
                    Field::UpdateCheck,
                    format!("a {} address is not reachable over HTTP", url.scheme()),
                ));
            }
            if url.host_str().is_none_or(str::is_empty) {
                return Err(Invalid::new(
                    Field::UpdateCheck,
                    "it names no host to reach",
                ));
            }
        }
        Ok(())
    }
}
