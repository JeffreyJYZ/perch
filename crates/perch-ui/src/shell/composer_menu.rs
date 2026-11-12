//! The command list above the composer: what typing `/` opens, and the keys that steer it.
//!
//! The list is not a second command table: every row comes from [`commands`](super::commands),
//! filtered by what follows the slash. A key that takes a row writes the command's typed form
//! into the prompt — it never runs it — so the argument can be finished by hand before Enter.

use rune::{Key, KeyEvent, TextEdit};

use super::commands::{Command, filter_of, matching};
use super::live::Live;
use super::state::Shell;

/// Whether the list is open, and which row is picked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Menu {
    open: bool,
    index: usize,
}

impl<L: Live> Shell<L> {
    /// The rows the list would show: open, on a bare command being typed, filtered.
    ///
    /// `None` is a closed list, so a caller draws nothing rather than an empty box.
    pub fn command_menu(&self) -> Option<Vec<&'static Command>> {
        if !self.menu.open {
            return None;
        }
        let filter = filter_of(&self.prompt.text)?;
        Some(matching(filter))
    }

    /// Which row is picked, clamped to the list that is showing.
    pub fn menu_index(&self) -> usize {
        let len = self.command_menu().map_or(0, |rows| rows.len());
        if len == 0 {
            0
        } else {
            self.menu.index.min(len - 1)
        }
    }

    /// Opens the list on the empty composer that `/` was typed into.
    pub(super) fn open_menu(&mut self) {
        self.menu.open = true;
        self.menu.index = 0;
    }

    /// Puts a slash in the empty composer and opens the list above it.
    ///
    /// The key is consumed and the field never sees it: the caret is placed after the slash here,
    /// so narrowing the list by typing works on the real field from the next press on. The field's
    /// focus belongs to the running window, not to this replacement, so it is carried over — a
    /// new `TextEdit` would be an unfocused copy and the next key would go nowhere.
    pub(super) fn start_command(&mut self) {
        let focused = self.prompt.focused;
        self.prompt = edit("/");
        self.prompt.focused = focused;
        self.open_menu();
    }

    /// Closes the list and keeps the text: Escape leaves the half-typed command in the box.
    pub fn close_menu(&mut self) {
        self.menu.open = false;
        self.menu.index = 0;
    }

    /// Picks the row at `index`, as a press on it does before it is taken.
    pub fn command_menu_pick(&mut self, index: usize) {
        self.menu.index = index;
    }

    /// Whether the list is drawn. A closed list is not the same as one with no row: an unknown
    /// prefix still draws "no command starts with that".
    pub fn menu_open(&self) -> bool {
        self.menu.open
    }

    /// The presses the open list answers before the composer's own map.
    ///
    /// Down and Up move the pick, Tab and Enter write the picked command into the prompt, and
    /// Escape closes the list without touching what was typed. Every other key falls through, so
    /// a letter narrows the list through the field's own edit.
    pub fn menu_key(&mut self, ev: &KeyEvent) -> bool {
        if !self.menu.open || ev.mods.cmd || ev.mods.alt {
            return false;
        }
        // Text that is no longer a bare command closes the list on the next press: the space that
        // starts an argument, a newline, or a backspace past the slash.
        if filter_of(&self.prompt.text).is_none() {
            self.close_menu();
            return false;
        }
        match &ev.key {
            Key::Down => {
                self.step(1);
                true
            }
            Key::Up => {
                self.step(-1);
                true
            }
            Key::Tab | Key::Enter => self.accept_menu(),
            // Escape closes the list and keeps the text, which is the one thing it must not do.
            Key::Escape => {
                self.close_menu();
                true
            }
            _ => false,
        }
    }

    /// Writes the picked command into the prompt and closes the list. Reports whether one was
    /// picked: an empty list has nothing to take.
    pub fn accept_menu(&mut self) -> bool {
        let Some(row) = self
            .command_menu()
            .and_then(|rows| rows.get(self.menu_index()).copied())
        else {
            return false;
        };
        let mut text = format!("/{}", row.name);
        if row.arg.is_some() {
            text.push(' ');
        }
        let focused = self.prompt.focused;
        self.prompt = edit(&text);
        self.prompt.focused = focused;
        self.close_menu();
        true
    }

    /// Moves the pick by `step`, wrapping at both ends of the list.
    fn step(&mut self, step: isize) {
        let len = self.command_menu().map_or(0, |rows| rows.len()) as isize;
        if len <= 0 {
            return;
        }
        let index = (self.menu_index() as isize + step).rem_euclid(len);
        self.menu.index = index as usize;
    }
}

/// A field holding `text`, with the caret after it.
pub(super) fn edit(text: &str) -> TextEdit {
    let mut edit = TextEdit::default();
    edit.text = text.to_string();
    edit.anchor = text.len();
    edit.focus = text.len();
    edit
}

#[cfg(test)]
mod tests {
    use rune::{Key, KeyEvent, Mods};

    use super::Shell;
    use crate::shell::COMMANDS;

    fn press(key: Key) -> KeyEvent {
        KeyEvent {
            key,
            mods: Mods::default(),
        }
    }

    #[test]
    fn an_open_list_steps_takes_and_closes_without_clearing() {
        let mut shell = Shell::idle(Vec::new(), 0);
        shell.open_menu();
        shell.set_prompt("/");
        assert_eq!(shell.command_menu().unwrap().len(), COMMANDS.len());

        assert!(shell.menu_key(&press(Key::Down)));
        assert_eq!(shell.menu_index(), 1);
        assert!(shell.menu_key(&press(Key::Up)));
        assert_eq!(shell.menu_index(), 0);

        assert!(shell.menu_key(&press(Key::Escape)));
        assert!(!shell.menu_open());
        assert_eq!(shell.prompt_text(), "/", "Escape keeps what was typed");

        shell.open_menu();
        shell.set_prompt("/rename");
        assert!(shell.menu_key(&press(Key::Tab)));
        assert_eq!(shell.prompt_text(), "/rename ");
        assert!(!shell.menu_open());
    }
}
