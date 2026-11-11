//! The palette's presses: what a row is, how the pick moves, and what taking one does.
//!
//! Keyboard and pointer run this one path, so a row taken with Enter and a row pressed with the
//! pointer cannot mean two things.

use rune::{Key, KeyEvent};

use super::live::Live;
use super::palette::{Palette, Row};
use super::state::Shell;

impl<L: Live> Shell<L> {
    /// Whether the palette is the pane showing.
    pub fn palette_open(&self) -> bool {
        matches!(self.pane, super::pane::Pane::Palette)
    }

    /// Shows the palette, or returns to the session when it is already showing.
    pub fn toggle_palette(&mut self) {
        if self.palette_open() {
            self.close_palette();
            return;
        }
        self.close_menu();
        let mut palette = Palette::default();
        // The filter has the keyboard the moment the palette opens: a list you must click
        // before typing reads as broken.
        palette.query.focused = true;
        self.palette = Some(palette);
        self.pane = super::pane::Pane::Palette;
    }

    /// Hides the palette and returns to the transcript, dropping the filter.
    pub fn close_palette(&mut self) {
        self.palette = None;
        self.back_to_session();
    }

    /// Every row the filter keeps: commands first, then the app actions.
    pub fn palette_rows(&self) -> Vec<Row> {
        let needle = self.palette.as_ref().map_or_else(String::new, |palette| {
            palette.query.text.trim().to_lowercase()
        });
        let keeps = |label: &str| needle.is_empty() || label.to_lowercase().contains(&needle);
        let mut rows: Vec<Row> = super::commands::COMMANDS
            .iter()
            .filter(|command| keeps(command.name))
            .map(Row::Command)
            .collect();
        rows.extend(
            super::palette::ACTIONS
                .iter()
                .filter(|action| keeps(action.label))
                .map(Row::Action),
        );
        rows
    }

    /// Which row is picked, clamped to the rows the filter keeps.
    pub fn palette_index(&self) -> usize {
        let len = self.palette_rows().len();
        if len == 0 {
            0
        } else {
            self.palette
                .as_ref()
                .map_or(0, |palette| palette.index)
                .min(len - 1)
        }
    }

    /// The presses the palette answers: Escape closes, Up and Down move, Enter takes the pick.
    /// Every other press is the filter field's, so a letter narrows the list as it is typed.
    pub fn palette_key(&mut self, ev: &KeyEvent) -> bool {
        if !self.palette_open() {
            return false;
        }
        match &ev.key {
            Key::Escape if !ev.mods.cmd && !ev.mods.alt => {
                self.close_palette();
                true
            }
            Key::Down => {
                self.step_palette(1);
                true
            }
            Key::Up => {
                self.step_palette(-1);
                true
            }
            Key::Enter if !ev.mods.shift => self.palette_take(),
            _ => false,
        }
    }

    /// Runs the row that is picked. A command that wants an argument is written into the prompt
    /// instead of run, so the person finishes it where they can see it.
    pub fn palette_take(&mut self) -> bool {
        let Some(row) = self.palette_rows().get(self.palette_index()).cloned() else {
            return false;
        };
        match row {
            Row::Command(command) if command.arg.is_some() => {
                self.close_palette();
                self.prompt = super::composer_menu::edit(&format!("/{} ", command.name));
                true
            }
            Row::Command(command) => {
                self.close_palette();
                self.run_command(&format!("/{}", command.name));
                true
            }
            Row::Action(action) => {
                self.close_palette();
                self.run_action(action)
            }
        }
    }

    /// Carries out one app action, which is the same press its control makes.
    fn run_action(&mut self, action: &super::palette::Action) -> bool {
        match action.label {
            "Settings" => {
                self.toggle_settings();
                true
            }
            "New session" => self.new_session(),
            "Toggle dock" => {
                self.dock_hidden = !self.dock_hidden;
                true
            }
            "Close tab" => self.close_showing_tab(),
            other => {
                self.say(&format!("{other} has no action behind it"));
                true
            }
        }
    }

    /// Closes the file tab the pane is showing, and says so when there is none.
    fn close_showing_tab(&mut self) -> bool {
        let path = match &self.pane {
            super::pane::Pane::File(path) => path.clone(),
            _ => {
                self.say("no file tab is showing, so there is nothing to close");
                return true;
            }
        };
        self.close_file(&path);
        true
    }

    fn step_palette(&mut self, step: isize) {
        let len = self.palette_rows().len() as isize;
        if len <= 0 {
            return;
        }
        let index = (self.palette_index() as isize + step).rem_euclid(len) as usize;
        if let Some(palette) = self.palette.as_mut() {
            palette.index = index;
        }
    }
}
