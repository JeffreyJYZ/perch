//! The finder's presses: ⌘P opens and closes it, Up and Down move the pick, Enter opens the file,
//! Escape leaves whatever was showing untouched.
//!
//! Opening reads the worktree once; every other press only reads the list it kept. A file taken
//! here goes through [`Shell::open_file`], which is the same path a tree row's press takes, so the
//! tab it opens and the pane it shows cannot differ.

use std::path::PathBuf;

use rune::{Key, KeyEvent};

use super::finder::{Finder, FinderRow};
use super::live::Live;
use super::state::Shell;

impl<L: Live> Shell<L> {
    /// Whether the file finder is showing.
    pub fn finder_open(&self) -> bool {
        self.finder.is_some()
    }

    /// ⌘P: opens the finder over the worktree, or closes it when it is already showing.
    ///
    /// The list is read here and only here, so a keystroke never walks the tree. The field takes
    /// the keyboard before the overlay is mounted, because a field that must be clicked before
    /// typing reads as broken.
    pub fn toggle_finder(&mut self) {
        if self.finder_open() {
            self.close_finder();
            return;
        }
        self.close_menu();
        let (files, truncated) = self
            .workspace
            .as_ref()
            .map_or_else(|| (Vec::new(), false), super::finder_scan::scan);
        let mut finder = Finder::read(files, truncated);
        finder.query.focused = true;
        self.finder = Some(finder);
    }

    /// Hides the finder. The pane under it is not touched: Escape closes the overlay and leaves
    /// whatever was open exactly as it was.
    pub fn close_finder(&mut self) {
        self.finder = None;
    }

    /// The rows the field keeps, best first.
    pub fn finder_rows(&self) -> Vec<FinderRow> {
        self.finder.as_ref().map(Finder::rows).unwrap_or_default()
    }

    /// How many files matched, before the row cap.
    pub fn finder_total(&self) -> usize {
        self.finder.as_ref().map_or(0, Finder::total)
    }

    /// Whether the scan stopped at its file cap, so the overlay can say the list is short.
    pub fn finder_truncated(&self) -> bool {
        self.finder.as_ref().is_some_and(|finder| finder.truncated)
    }

    /// Every file the scan read, as the overlay lists it. Empty when it is not open.
    pub fn finder_files(&self) -> Vec<PathBuf> {
        self.finder
            .as_ref()
            .map(|finder| finder.files.clone())
            .unwrap_or_default()
    }

    /// What the field holds, as typed.
    pub fn finder_query_text(&self) -> String {
        self.finder
            .as_ref()
            .map(|finder| finder.query.text.clone())
            .unwrap_or_default()
    }

    /// Which row is picked, clamped to the rows the field keeps.
    pub fn finder_index(&self) -> usize {
        self.finder.as_ref().map_or(0, Finder::index)
    }

    /// Picks the row at `index`, which is what a press on a row means before it is taken.
    pub fn finder_pick(&mut self, index: usize) {
        if let Some(finder) = self.finder.as_ref() {
            finder.pick(index);
        }
    }

    /// Opens the picked file in its tab and closes the overlay. A list with nothing picked opens
    /// nothing, rather than a file the field never showed.
    pub fn finder_take(&mut self) -> bool {
        let Some(row) = self.finder_rows().get(self.finder_index()).cloned() else {
            return false;
        };
        self.finder = None;
        self.open_file(&row.path);
        true
    }

    /// The presses the finder answers: Escape closes, Up and Down move, Enter takes the pick.
    /// Every other press is the field's, so a letter narrows the list as it is typed.
    pub fn finder_key(&mut self, ev: &KeyEvent) -> bool {
        if !self.finder_open() || ev.mods.cmd || ev.mods.alt {
            return false;
        }
        match &ev.key {
            Key::Escape => {
                self.close_finder();
                true
            }
            Key::Down => {
                self.step_finder(1);
                true
            }
            Key::Up => {
                self.step_finder(-1);
                true
            }
            Key::Enter if !ev.mods.shift => self.finder_take(),
            _ => false,
        }
    }

    /// Moves the pick by `step`, wrapping at both ends.
    fn step_finder(&mut self, step: isize) {
        if let Some(finder) = self.finder.as_ref() {
            finder.step(step);
        }
    }
}
