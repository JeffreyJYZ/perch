//! The chords the window answers to, and the ones a person changed.
//!
//! The defaults are the chords the window has always used: ⌘, for the settings, ⌘K for the
//! palette, ⌘P for the finder, ⌘F for the transcript's search, ⌘O for a card's whole output and
//! ⌘Enter for allowing a call for the session. A saved binding replaces one of them, and the
//! chords live in `settings.toml`'s `[keybinds]` table, one `intent = "chord"` pair each.

use super::keymap::Intent;

mod bindings;
mod panel;
#[cfg(test)]
mod tests;

pub use bindings::Bindings;
pub use panel::Keybinds;

/// One intent a person can rebind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bindable {
    /// Show the settings panel.
    Settings,
    /// Show the command palette.
    Palette,
    /// Show the file finder.
    Finder,
    /// Search the transcript.
    Find,
    /// Open a tool card's whole output.
    Open,
    /// Allow the waiting call for the rest of the session.
    AllowForSession,
}

impl Bindable {
    /// Every intent that can be rebound, in the order the pane lists them.
    pub const ALL: [Bindable; 6] = [
        Bindable::Settings,
        Bindable::Palette,
        Bindable::Finder,
        Bindable::Find,
        Bindable::Open,
        Bindable::AllowForSession,
    ];

    /// The name the file uses for this intent.
    pub fn name(self) -> &'static str {
        match self {
            Bindable::Settings => "settings",
            Bindable::Palette => "palette",
            Bindable::Finder => "finder",
            Bindable::Find => "find",
            Bindable::Open => "open_output",
            Bindable::AllowForSession => "allow_for_session",
        }
    }

    /// The name a person reads beside the field.
    pub fn label(self) -> &'static str {
        match self {
            Bindable::Settings => "Settings",
            Bindable::Palette => "Command palette",
            Bindable::Finder => "File finder",
            Bindable::Find => "Search transcript",
            Bindable::Open => "Open tool output",
            Bindable::AllowForSession => "Allow for session",
        }
    }

    /// The chord the window uses when nothing was saved.
    pub fn default_chord(self) -> &'static str {
        match self {
            Bindable::Settings => "mod+,",
            Bindable::Palette => "mod+k",
            Bindable::Finder => "mod+p",
            Bindable::Find => "mod+f",
            Bindable::Open => "mod+o",
            Bindable::AllowForSession => "mod+enter",
        }
    }

    /// The intent the window means.
    pub fn intent(self) -> Intent {
        match self {
            Bindable::Settings => Intent::Settings,
            Bindable::Palette => Intent::Palette,
            Bindable::Finder => Intent::Finder,
            Bindable::Find => Intent::Find,
            Bindable::Open => Intent::Open,
            Bindable::AllowForSession => Intent::AllowForSession,
        }
    }
}

/// What the pane shows about one binding: the chord it answers to, and why a saved one is not
/// used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeybindView {
    /// The intent's name, as the file spells it.
    pub name: String,
    /// The name a person reads.
    pub label: String,
    /// The chord the intent answers to now.
    pub chord: String,
    /// Why a saved chord could not be used, when one could not.
    pub refusal: Option<String>,
}
