//! Forking the session on screen: a second column holding the same conversation.
//!
//! The copy is opened through the same path a session read from disk takes at start, so its
//! history is replayed into the column immediately and its first turn continues from it. The
//! session it was taken from is not touched: it stays open with its own agent, and a turn it runs
//! after the fork lands in its own log.

use perch_store::SessionLog;

use super::boot::runtime;
use super::{AppState, Error, Result};

impl AppState {
    /// Forks the session at `index` whole and shows the copy, answering its index.
    ///
    /// A whole fork is every event the source's log holds, because the person asked for the
    /// session as it stands. A boundary fork — the first exchange only — is
    /// [`SessionLog::fork`]'s `at`; the window has no key for it yet, so nothing here passes one.
    pub fn fork_session(&mut self, index: usize) -> Result<usize> {
        let Some(source) = self.sessions.get(index) else {
            return Err(Error::MissingSession(index));
        };
        let from = source.meta.clone();
        let meta = SessionLog::fork(&self.dir, &from, None)?;
        let words = self.snapshot.words.clone();
        let runtime = runtime(&self.dir, meta, &self.live, &self.defaults, &words)?;
        self.sessions.push(runtime);
        self.active = self.sessions.len() - 1;
        Ok(self.active)
    }
}
