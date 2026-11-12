//! The one rule about what may be saved.
//!
//! Split from [`settings`](super::settings) so the draft and the fields it holds stay one
//! readable file and the checks are another.

use std::collections::BTreeMap;

use perch_store::settings::Chord;
use url::Url;

use crate::theme::Library;

use super::{Field, Invalid, SettingsDraft};

impl SettingsDraft {
    /// Checks every field, in the order the panel shows them.
    ///
    /// An address has to parse as a URL with an HTTP scheme and a host, because the provider
    /// client appends `/chat/completions` to it: anything else would save and then fail every
    /// turn. A model has to be named, because an empty one is not a model the server knows. The
    /// theme has to be one this machine can draw — a built-in or a file that parsed — because
    /// anything else cannot be drawn. The font size is allowed to be empty, which is the default
    /// scale, and otherwise must be a number; a number outside the scale's bounds is clamped by
    /// [`Self::font_points`] rather than refused, and the pane says where it landed. The update
    /// check is allowed to be empty, which is the app never reaching the network, and otherwise is
    /// held to the same URL rule as the provider's address. A chord has to be one the grammar can
    /// read, and one chord may answer one intent only.
    pub fn validate(&self) -> Result<(), Invalid> {
        self.validate_with(&Library::discover())
    }

    /// [`Self::validate`] against a library a frame already read, so a save reads the themes
    /// directory once.
    pub fn validate_with(&self, themes: &Library) -> Result<(), Invalid> {
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
                "pick one from the provider's list",
            ));
        }
        if let Err(words) = themes.choose(&self.theme) {
            return Err(Invalid {
                field: Field::Theme,
                words,
            });
        }
        if !self.font_size.trim().is_empty() && self.font_points().is_none() {
            return Err(Invalid::new(
                Field::FontSize,
                "must be a number in points, for example 13",
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
        self.validate_keybinds()?;
        Ok(())
    }

    /// The size to store: the typed points clamped to the scale's own bounds, or `None` for the
    /// default scale. Validation has already refused text that is not a number, so this only
    /// decides the clamped value — a size of 8 saves as 10, which is what the pane says it will
    /// draw.
    pub fn font_points(&self) -> Option<f32> {
        let points: f32 = self.font_size.trim().parse().ok()?;
        points.is_finite().then(|| crate::kit::clamp_size(points).0)
    }

    /// Checks the chords the person typed, and refuses one that cannot be read or that two
    /// intents want.
    fn validate_keybinds(&self) -> Result<(), Invalid> {
        let mut held: BTreeMap<String, String> = BTreeMap::new();
        for (intent, text) in &self.keybinds {
            let chord = text.trim();
            if chord.is_empty() {
                continue;
            }
            if let Err(words) = Chord::parse(chord) {
                return Err(Invalid::new(
                    Field::Keybind,
                    format!("{chord} is not a chord: {words}"),
                ));
            }
            if let Some(other) = held.get(chord) {
                return Err(Invalid::new(
                    Field::Keybind,
                    format!("{chord} is bound to {intent} and to {other}"),
                ));
            }
            held.insert(chord.to_string(), intent.clone());
        }
        Ok(())
    }
}
