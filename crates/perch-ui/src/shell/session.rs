//! Starting a session: on the worktree the window already shows, with no picker in the way.

use super::live::Live;
use super::state::Shell;

impl<L: Live> Shell<L> {
    /// Opens a session on the worktree this window already has open, and shows it.
    ///
    /// A session belongs to a directory that exists, and the open one is that directory: a
    /// picker would only offer paths the window is not showing. The app makes the session and
    /// answers with its index and header, so the list grows without a reload from disk.
    pub fn new_session(&mut self) -> bool {
        let Some(worktree) = self
            .sessions
            .get(self.active)
            .map(|session| session.worktree.clone())
            .or_else(|| {
                self.workspace
                    .as_ref()
                    .map(|workspace| workspace.root().to_path_buf())
            })
        else {
            return false;
        };
        let Some(live) = self.live.as_mut() else {
            return false;
        };
        let Ok((index, meta)) = live.new_session(&worktree) else {
            return false;
        };
        self.sessions.push(meta);
        // The app made this session active; an index past the end would name one the list does
        // not hold, so it is clamped to the row that was just added.
        self.active = index.min(self.sessions.len() - 1);
        self.close_settings();
        self.refresh();
        true
    }
}
