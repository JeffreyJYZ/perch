//! The command palette: one list over the command registry and the app actions beside it.
//!
//! ⌘K opens it, typing filters it, Up and Down move the pick, Enter takes it, Escape closes it.
//! The commands are [`commands`](super::commands) itself — never a copy — so a command added
//! there is in the palette on the next frame; the app actions are the few things the window does
//! that have no slash name. The presses that take a row live in
//! [`palette_take`](super::palette_take).

use rune::TextEdit;

use super::commands::Command;

/// One thing the palette offers that has no slash command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Action {
    /// What the row says.
    pub label: &'static str,
    /// One line saying what it does.
    pub about: &'static str,
}

/// The app actions the palette adds to the commands.
pub const ACTIONS: &[Action] = &[
    Action {
        label: "Settings",
        about: "open the settings pane",
    },
    Action {
        label: "New session",
        about: "start a session on this worktree",
    },
    Action {
        label: "Toggle dock",
        about: "hide or show the terminal dock",
    },
    Action {
        label: "Close tab",
        about: "close the file tab that is showing",
    },
];

/// The palette's state: the filter field and which row is picked.
pub(super) struct Palette {
    pub(super) query: TextEdit,
    pub(super) index: usize,
}

impl Default for Palette {
    fn default() -> Self {
        Palette {
            query: TextEdit::default(),
            index: 0,
        }
    }
}

/// One row of the palette: a command, or an app action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// A registered command.
    Command(&'static Command),
    /// An app action beside the commands.
    Action(&'static Action),
}

impl Row {
    /// What the row says: the command's typed form, or the action's label.
    pub fn label(&self) -> String {
        match self {
            Row::Command(command) => command.label(),
            Row::Action(action) => action.label.to_string(),
        }
    }

    /// The line under the row's name.
    pub fn about(&self) -> &'static str {
        match self {
            Row::Command(command) => command.about,
            Row::Action(action) => action.about,
        }
    }
}
