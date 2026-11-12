//! The settings a person edits: the provider's address and model, the theme, and the API key.
//!
//! [`SettingsDraft`] is the form's data and [`SettingsDraft::validate`] is the one rule about what
//! may be saved, so a bad field is refused before anything is written. The key is deliberately not
//! a field of [`Settings`](perch_store::Settings): it goes to the secret store, which is why a
//! draft can be written to `settings.toml` and never carry a secret with it.

use std::collections::BTreeMap;

mod validate;

/// The theme a fresh install uses, and the one the panel offers when nothing is saved yet.
pub const DEFAULT_THEME: &str = "dark";

/// The two themes the panel's picker offers beside the files a person adds.
///
/// Saving is held to [`Library`], not to these: a theme file under `$PERCH_HOME` is a name the
/// window can draw, and refusing it here would drop a choice the person just made. The picker
/// itself is the shell's, and it lists what it is given.
pub const THEMES: [&str; 2] = ["dark", "light"];

/// Which field a refusal is about, so the panel can name it and mark the right box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    /// The provider's API root.
    BaseUrl,
    /// The model every new session asks for.
    Model,
    /// One of [`THEMES`], or a theme file this machine has.
    Theme,
    /// The points the type scale is drawn at, empty for the default.
    FontSize,
    /// The release manifest the app checks, when one is named.
    UpdateCheck,
    /// The command a new terminal tab runs.
    Shell,
    /// A chord an intent answers to.
    Keybind,
}

impl Field {
    /// The field's name as a person reads it, and the word a refusal starts with.
    pub fn words(self) -> &'static str {
        match self {
            Field::BaseUrl => "base URL",
            Field::Model => "model",
            Field::Theme => "theme",
            Field::FontSize => "font size",
            Field::UpdateCheck => "update check URL",
            Field::Shell => "shell",
            Field::Keybind => "keybind",
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
    /// One of [`THEMES`], or a name the theme library has: a file under `$PERCH_HOME`.
    pub theme: String,
    /// The points the type scale is drawn at, empty for the default scale.
    pub font_size: String,
    /// The release manifest to check, empty when the app must not reach the network.
    pub update_check: String,
    /// The command a new terminal tab runs, empty for this machine's own shell.
    ///
    /// A command line, never a path: quoting is the person's, and an empty field is the default
    /// rather than an error.
    pub shell: String,
    /// A key to store, empty when the person typed none.
    pub api_key: String,
    /// The provider profile this draft edits: the name of the profile the panel has selected.
    ///
    /// Empty means the active profile, which is what every save from before the list existed
    /// did.
    pub profile: String,
    /// Whether the edited profile becomes the active one on this save.
    pub activate: bool,
    /// The chords the person rebound, by intent name: `settings = "mod+,"`.
    ///
    /// Empty when nothing was typed, and an empty table is not written to `settings.toml` at
    /// all, so a file nobody touched stays exactly what it was.
    pub keybinds: BTreeMap<String, String>,
}
