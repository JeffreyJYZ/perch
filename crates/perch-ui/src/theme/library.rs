//! The directory read: what is built in, what is on disk, and what a saved name resolves to.

use std::path::{Path, PathBuf};

use super::parse;
use super::{BUILTIN, Library, State, Theme};
use crate::kit;

impl Library {
    /// The library this machine has, read from `$PERCH_HOME` or `$HOME`.
    pub fn discover() -> Self {
        Self::at(&super::dir())
    }

    /// The library in `dir`, which a test points at a temporary one.
    pub fn at(dir: &Path) -> Self {
        let mut themes: Vec<Theme> = BUILTIN
            .iter()
            .map(|name| Theme {
                name: name.to_string(),
                state: State::Ready(kit::builtin(*name == "dark")),
            })
            .collect();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Self { themes };
        };
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
            .collect();
        files.sort();
        for path in files {
            let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let state = match std::fs::read_to_string(&path) {
                Ok(text) => match parse::anchors(&text) {
                    Ok(anchors) => State::Ready(anchors),
                    Err(error) => State::Unavailable(error),
                },
                Err(error) => State::Unavailable(error.to_string()),
            };
            let theme = Theme {
                name: name.to_string(),
                state,
            };
            // A file that shadows a built-in wins: naming it is a deliberate act, so it replaces
            // the entry rather than listing the same name twice.
            match themes.iter_mut().find(|listed| listed.name == theme.name) {
                Some(listed) => *listed = theme,
                None => themes.push(theme),
            }
        }
        Self { themes }
    }

    /// Every theme, in listing order.
    pub fn themes(&self) -> &[Theme] {
        &self.themes
    }

    /// The names, as one line a refusal can quote: `dark, light, dusk`.
    pub fn names(&self) -> String {
        self.themes
            .iter()
            .map(|theme| theme.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The theme called `name`, ready or not.
    pub fn get(&self, name: &str) -> Option<&Theme> {
        let name = name.trim();
        self.themes.iter().find(|theme| theme.name == name)
    }

    /// What `/theme <name>` switches to, refusing one that is not there by naming what is.
    pub fn choose(&self, name: &str) -> Result<&Theme, String> {
        let name = name.trim();
        match self.get(name) {
            Some(theme) if theme.error().is_none() => Ok(theme),
            Some(theme) => Err(format!(
                "theme {name:?} will not load: {}",
                theme.error().unwrap_or_default()
            )),
            None => Err(format!(
                "theme {name:?} is not one of {}; /themes lists them",
                self.names()
            )),
        }
    }

    /// What `/themes` answers: every name, the active one marked, and every failure with its why.
    pub fn listing(&self, active: &str) -> Vec<String> {
        self.themes
            .iter()
            .map(|theme| {
                let mut line = theme.name.clone();
                if theme.name == active {
                    line.push_str(" (active)");
                }
                if let Some(error) = theme.error() {
                    line.push_str(&format!(" — will not load: {error}"));
                }
                line
            })
            .collect()
    }

    /// What a saved name draws: the theme actually used, its anchors, and the one line a pane says
    /// when the saved name could not be used as written.
    pub fn active(&self, saved: &str) -> (String, crate::kit::Anchors, Option<String>) {
        let name = saved.trim();
        match self.get(name) {
            Some(theme) => match theme.state {
                State::Ready(anchors) => (theme.name.clone(), anchors, None),
                State::Unavailable(ref error) => (
                    BUILTIN[0].to_string(),
                    kit::builtin(true),
                    Some(format!(
                        "theme {name:?} will not load: {error}; drawing dark"
                    )),
                ),
            },
            None => (
                BUILTIN[0].to_string(),
                kit::builtin(true),
                Some(format!(
                    "theme {name:?} is not one of {}; drawing dark",
                    self.names()
                )),
            ),
        }
    }
}
