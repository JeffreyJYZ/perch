//! Which pane the workspace draws, and the file tabs that name one.
//!
//! There is one value: [`Pane`]. The strip marks the tab that names it and the workspace draws it,
//! so a tab and the pane under it cannot disagree — there is no second flag deciding which of them
//! is showing. A file's tab is keyed by its path, which is why two files of one name in different
//! directories get a tab each, and why reopening a file focuses the tab it already has.

use std::path::{Path, PathBuf};

use super::live::Live;
use super::state::Shell;

/// The pane the centre column draws. Exactly one of them is showing at a time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pane {
    /// The transcript, and the prompt under it.
    Session,
    /// A file's read-only preview, at the path the tree selected.
    File(PathBuf),
    /// The settings form.
    Settings,
    /// The command palette: the commands and the app actions over the same registry.
    Palette,
}

/// The id a file's tab is drawn under: its own path, so no two files share one.
pub(super) fn tab_id(path: &Path) -> String {
    path.display().to_string()
}

/// The word a file's tab carries: its name, or the whole path when it has none.
pub(super) fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
mod tests;

impl<L: Live> Shell<L> {
    /// The one value the workspace draws from.
    pub fn pane(&self) -> &Pane {
        &self.pane
    }

    /// Every file tab, in the order it was opened.
    pub fn file_tabs(&self) -> &[PathBuf] {
        &self.files_open
    }

    /// Shows `path` in a tab of its own, adding one when it has none, and makes it the pane.
    ///
    /// Returns whether the file could be read: a deleted file still opens its tab, because the
    /// words saying why are the preview's and the transcript is one Escape away.
    pub fn open_file(&mut self, path: &Path) -> bool {
        let absolute = self.absolute(path);
        // The settings form is a pane of its own: its draft is dropped rather than hidden, so a
        // half-typed base URL is never saved by a file opening behind it. The palette goes for
        // the same reason: its filter is not a pane anyone returns to.
        self.close_settings();
        self.palette = None;
        if !self.files_open.contains(&absolute) {
            self.files_open.push(absolute.clone());
        }
        self.pane = Pane::File(absolute);
        self.files
            .as_mut()
            .is_some_and(|panel| panel.select(path).is_ok())
    }

    /// Focuses a tab the strip already carries, re-reading the file so the pane shows it.
    ///
    /// A path with no tab changes nothing: a press names a tab that is drawn, and a stale one must
    /// not invent a second.
    pub fn focus_file(&mut self, path: &Path) {
        let absolute = self.absolute(path);
        if !self.files_open.contains(&absolute) {
            return;
        }
        self.close_settings();
        self.palette = None;
        self.pane = Pane::File(absolute);
        if let Some(panel) = self.files.as_mut() {
            let _ = panel.select(path);
        }
    }

    /// Closes a file's tab, returning to the session when it was the one showing.
    ///
    /// The tree keeps its selection, so the same file is one press from coming back.
    pub fn close_file(&mut self, path: &Path) {
        let absolute = self.absolute(path);
        self.files_open.retain(|open| *open != absolute);
        if *self.pane() == Pane::File(absolute) {
            self.back_to_session();
        }
    }

    /// The strip's close press: closes the file tab `id`. Reports whether one closed.
    pub fn close_tab(&mut self, id: &str) -> bool {
        let Some(path) = self
            .files_open
            .iter()
            .find(|open| tab_id(open) == id)
            .cloned()
        else {
            return false;
        };
        self.close_file(&path);
        true
    }

    /// The strip's select press: shows the tab `id`, a session, a file, or the settings.
    pub fn show(&mut self, id: &str) {
        if id == super::SETTINGS {
            // A press on the settings tab while it is open leaves it open rather than flipping
            // it shut, which is what a tab does.
            if !self.settings_open() {
                self.toggle_settings();
            }
            return;
        }
        if let Some(index) = self.sessions.iter().position(|session| session.id == id) {
            // The session already showing is not a switch: its file tabs are the ones on screen,
            // so the press returns to the transcript and keeps them.
            if index == self.active {
                self.close_settings();
            } else {
                self.select(index);
            }
            return;
        }
        if let Some(path) = self
            .files_open
            .iter()
            .find(|open| tab_id(open) == id)
            .cloned()
        {
            self.focus_file(&path);
        }
    }

    /// Leaves whatever pane is showing for the session's transcript, keeping the tree's selection
    /// and every file tab. Reports whether there was a pane to leave.
    pub fn leave_pane(&mut self) -> bool {
        match self.pane {
            Pane::Session => false,
            Pane::Settings => {
                self.close_settings();
                true
            }
            Pane::Palette => {
                self.close_palette();
                true
            }
            Pane::File(_) => {
                self.back_to_session();
                true
            }
        }
    }

    /// The session's own pane: the one every other pane returns to.
    pub(super) fn back_to_session(&mut self) {
        self.pane = Pane::Session;
    }

    /// Every pane starts over for a session the window switched to: the settings draft and the
    /// finder's file list are dropped and the file tabs go, because they name files in the tree
    /// that was just replaced.
    pub(super) fn reset_panes(&mut self) {
        self.settings = None;
        self.palette = None;
        self.finder = None;
        self.pane = Pane::Session;
        self.files_open.clear();
    }

    /// `path` as this shell resolves it, with no walk of the filesystem.
    fn absolute(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            return path.to_path_buf();
        }
        self.workspace.as_ref().map_or_else(
            || path.to_path_buf(),
            |workspace| workspace.root().join(path),
        )
    }
}
