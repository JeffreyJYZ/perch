//! The commands that act on a session: a new one, its title, its transcript, and its removal.
//!
//! Split from [`commands_run`](super::commands_run) so the dispatch table stays one screen. Each
//! returns the sentence the transcript says, or the refusal naming what is missing.

use super::commands_export::export;
use super::live::Live;
use super::state::Shell;

impl<L: Live> Shell<L> {
    /// `/new`: a session on the worktree this window already shows.
    pub(super) fn command_new(&mut self) -> Result<Option<String>, String> {
        if !self.new_session() {
            return Err("no worktree is open, so no session was started".to_string());
        }
        let opened = self
            .sessions
            .get(self.active)
            .map(|session| session.title.clone())
            .unwrap_or_default();
        Ok(Some(format!("new session: {opened}")))
    }

    /// `/fork`: the conversation on screen, copied into a session of its own.
    pub(super) fn command_fork(&mut self) -> Result<Option<String>, String> {
        let index = self.active;
        let (index, meta) = match self.live.as_mut() {
            Some(live) => live.fork_session(index)?,
            None => return Err("no app is running, so no session was forked".to_string()),
        };
        let title = meta.title.clone();
        self.sessions.push(meta);
        self.active = index.min(self.sessions.len().saturating_sub(1));
        self.reopen_files();
        self.reset_panes();
        self.refresh();
        Ok(Some(format!("forked: {title}")))
    }

    /// `/rename <title>`: the name the session list and its log carry from now on.
    pub(super) fn command_rename(&mut self, arg: &str) -> Result<Option<String>, String> {
        if arg.trim().is_empty() {
            return Err("rename needs a title: /rename <title>".to_string());
        }
        let index = self.active;
        let Some(live) = self.live.as_mut() else {
            return Err("no app is running, so the title was not saved".to_string());
        };
        let meta = live.rename_session(index, arg.trim())?;
        if let Some(slot) = self.sessions.get_mut(index) {
            *slot = meta;
        }
        Ok(Some(format!("renamed: {}", arg.trim())))
    }

    /// `/export`: this session's transcript, under the data directory.
    pub(super) fn command_export(&mut self) -> Result<Option<String>, String> {
        let Some(dir) = self.live.as_ref().and_then(Live::data_dir) else {
            return Err("no data directory is known, so nothing was exported".to_string());
        };
        let Some(meta) = self.sessions.get(self.active).cloned() else {
            return Err("no session is open, so there is nothing to export".to_string());
        };
        let path = export(&dir, &meta, &self.transcript)?;
        Ok(Some(format!("exported: {}", path.display())))
    }

    /// `/clean`: empties the column, and says the session it could not.
    pub(super) fn command_clean(&mut self) -> Result<Option<String>, String> {
        if self.sessions.get(self.active).is_none() {
            return Err("no session is open, so there is nothing to clean".to_string());
        }
        if let Some(transcript) = self.live.as_mut().and_then(Live::transcript_mut) {
            transcript.clear();
        }
        self.transcript.clear();
        Ok(Some("cleaned: the transcript is empty".to_string()))
    }

    /// The row's delete control: the first press arms it, the second removes the session.
    ///
    /// A removed session stops its bridge and drops its runtime in the app, and its log is
    /// deleted. The window then shows the session the app says is active and re-reads the tree.
    pub(super) fn confirm_delete(&mut self, index: usize) -> bool {
        let Some(session) = self.sessions.get(index) else {
            return false;
        };
        if self.pending_delete != Some(index) {
            let title = session.title.clone();
            self.pending_delete = Some(index);
            self.say(&format!("delete {title}? press delete again to remove it"));
            return false;
        }
        let Some(live) = self.live.as_mut() else {
            self.say("no app is running, so the session was not deleted");
            return false;
        };
        match live.delete_session(index) {
            Ok(active) => {
                self.pending_delete = None;
                self.sessions.remove(index);
                self.active = active.min(self.sessions.len().saturating_sub(1));
                self.reopen_files();
                self.reset_panes();
                self.refresh();
                true
            }
            Err(words) => {
                self.say(&words);
                false
            }
        }
    }
}
