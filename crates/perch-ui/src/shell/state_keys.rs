//! The state a command asks the window for, and the one-shot requests it leaves for the press.
//!
//! Split from [`state`](super::state) so the fields, the constructors and the read-only accessors
//! stay three readable files. Nothing here decides anything: a command sets a flag, and the press
//! handler that has a `Tx` takes it.

use perch_store::SessionMeta;

use super::live::Live;
use super::state::Shell;

impl<L: Live> Shell<L> {
    /// Every session's header as the list draws it.
    ///
    /// With an app behind the window it is the app's copy, which is where a branch re-read after
    /// a checkout and a title written by an auto-name land; with no app it is the copy the list
    /// was built with.
    pub fn headers(&self) -> Vec<SessionMeta> {
        if self.live.is_none() {
            return self.sessions.clone();
        }
        (0..self.sessions.len())
            .map(|index| {
                self.live
                    .as_ref()
                    .and_then(|live| live.session(index))
                    .unwrap_or_else(|| self.sessions[index].clone())
            })
            .collect()
    }

    /// How many sessions the window lists.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Whether the terminal dock is hidden.
    pub fn dock_hidden(&self) -> bool {
        self.dock_hidden
    }

    /// Whether the keyboard is in the session list.
    pub fn sessions_focused(&self) -> bool {
        self.sessions_focused
    }

    /// Whether `/quit` asked the window to close.
    pub fn quit_requested(&self) -> bool {
        self.quit
    }

    /// Takes the close request once, so one command closes one window.
    pub fn take_quit(&mut self) -> bool {
        std::mem::take(&mut self.quit)
    }

    /// Takes the focus request `/sessions` made, so the press handler focuses the list once.
    pub fn take_focus_sessions(&mut self) -> bool {
        std::mem::take(&mut self.focus_sessions)
    }

    /// Returns the keyboard to the prompt, leaving the session list.
    pub fn leave_sessions(&mut self) {
        self.sessions_focused = false;
    }

    /// A character reached the composer, which is what takes the keyboard back from the list and
    /// what disarms a delete a stray click armed.
    pub(super) fn typed(&mut self) {
        self.sessions_focused = false;
        self.pending_delete = None;
    }
}
