//! `/undo`: putting the files of the last turn back, as they were when it began.
//!
//! The worktree is captured at the start of every turn and kept beside the session, so a restore
//! needs nothing from the transcript: the set is the difference between that tree and the worktree
//! now, which catches a file written by `patch` or by a shell call as surely as one written by
//! `write`. The conversation is deliberately untouched — a provider's context cannot be unwound
//! without asking the model again, and editing items out of the transcript to look like a revert
//! would be a lie about what was said.

use crate::AppState;

impl AppState {
    /// Puts the last turn's files back, answering one line per path for the transcript.
    pub fn undo_turn(&mut self) -> Result<Vec<String>, String> {
        let Some(session) = self.sessions.get_mut(self.active) else {
            return Err("no session is open, so there is nothing to undo".to_string());
        };
        let running = session.is_running();
        let worktree = session.meta.worktree.clone();
        session.undo.restore(&worktree, running)
    }
}
