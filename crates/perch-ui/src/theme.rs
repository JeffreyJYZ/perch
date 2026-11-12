//! The theme library: the two themes every install has, and the files beside them.
//!
//! A theme is a TOML file in `$PERCH_HOME/.config/perch/themes/<name>.toml` (else under `$HOME`)
//! naming three anchors — `bg`, `ink`, `signal` — in OKLCH or as `#rrggbb`, plus optional `ok` and
//! `fail`. Every other tone is derived by [`from_anchors`](crate::kit::from_anchors), because a
//! theme that could set all twenty tones would drift from the type scale and the hairlines.
//!
//! The two built-ins are always present. A file that will not parse is still listed, with its
//! error: a theme that does not appear and gives no reason is the worst outcome.

mod library;
mod parse;

use std::path::{Path, PathBuf};

use crate::kit::{self, Anchors, clamp_size};
use rune_kit::Kit;

/// The themes every install has, in the order they are listed.
pub const BUILTIN: [&str; 2] = ["dark", "light"];

/// The directory the theme files live in, under a home directory.
pub const DIR: &str = "themes";

/// One entry in the library.
#[derive(Clone, Debug)]
pub struct Theme {
    /// What `/theme` takes: the file's stem, or a built-in's name.
    pub name: String,
    pub state: State,
}

/// Whether a theme can draw, and why not when it cannot.
#[derive(Clone, Debug)]
pub enum State {
    Ready(Anchors),
    Unavailable(String),
}

impl Theme {
    /// The three anchors, when the file parsed.
    pub fn anchors(&self) -> Option<Anchors> {
        match self.state {
            State::Ready(anchors) => Some(anchors),
            State::Unavailable(_) => None,
        }
    }

    /// The one line saying why this theme cannot draw, when it cannot.
    pub fn error(&self) -> Option<&str> {
        match &self.state {
            State::Ready(_) => None,
            State::Unavailable(error) => Some(error),
        }
    }
}

/// Every theme this machine has: the two built in, then the files, in name order.
///
/// [`Library::discover`] is the one a window reads; [`Library::at`] is the same over a directory a
/// test names.
#[derive(Clone, Debug)]
pub struct Library {
    themes: Vec<Theme>,
}

/// What the frame draws: the theme a saved name resolves to, the size, and the one line a pane
/// says when a saved value could not be used as written.
#[derive(Clone, Debug)]
pub struct Appearance {
    /// The theme actually drawn, which is `dark` when the saved name was not usable.
    pub theme: String,
    pub anchors: Anchors,
    /// The size after clamping, so what is drawn and what is stored agree.
    pub font_size: Option<f32>,
    pub note: Option<String>,
}

impl Appearance {
    /// The built-in dark appearance: what a shell with no app behind it draws with.
    pub fn dark() -> Self {
        Self::builtin("dark", true)
    }

    /// The built-in light appearance, for a shell whose saved word is `light` and no app states
    /// more than that.
    pub fn light() -> Self {
        Self::builtin("light", false)
    }

    /// A built-in theme by name, at the app's own type size.
    fn builtin(name: &str, dark: bool) -> Self {
        Self {
            theme: name.to_string(),
            anchors: kit::builtin(dark),
            font_size: None,
            note: None,
        }
    }

    /// Reads a saved theme name and size against `library`.
    pub fn resolve(library: &Library, saved: &str, font_size: Option<f32>) -> Self {
        let (theme, anchors, note) = library.active(saved);
        let mut notes: Vec<String> = note.into_iter().collect();
        let font_size = font_size.map(|size| {
            let (clamped, line) = clamp_size(size);
            notes.extend(line);
            clamped
        });
        Self {
            theme,
            anchors,
            font_size,
            note: (!notes.is_empty()).then(|| notes.join("; ")),
        }
    }

    /// The kit this draws.
    pub fn kit(&self) -> Kit {
        kit::from_anchors(&self.anchors, self.font_size)
    }
}

/// The library this machine has, read from `$PERCH_HOME` or `$HOME`.
pub fn discover() -> Library {
    Library::discover()
}

/// The theme files: `$PERCH_HOME/.config/perch/themes`, else `$HOME`'s.
pub fn dir() -> PathBuf {
    let home = std::env::var_os("PERCH_HOME")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default();
    dir_in(&home)
}

/// The theme files under `home`: `.config/perch/themes`.
pub fn dir_in(home: &Path) -> PathBuf {
    home.join(".config").join("perch").join(DIR)
}

#[cfg(test)]
mod tests {
    use super::dir_in;
    use std::path::Path;

    #[test]
    fn the_files_live_under_a_home_and_not_beside_the_sessions() {
        assert_eq!(
            dir_in(Path::new("/home/jyz")),
            Path::new("/home/jyz/.config/perch/themes")
        );
    }
}
