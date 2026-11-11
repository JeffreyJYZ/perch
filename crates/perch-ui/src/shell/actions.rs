//! What a press does to the state: a session switched, a turn sent or stopped, an answer given,
//! a terminal tab opened or shown.

use perch_files::Workspace;
use perch_term::TermSession;
use rune::TextEdit;

use super::live::Live;
use super::state::Shell;
use super::terminal::TerminalTab;
use crate::files::FilePanel;

impl<L: Live> Shell<L> {
    /// Selects the session at `index`. An index past the end is ignored rather than clamped: it
    /// names a session that is gone, and switching to a neighbour would switch to the wrong one.
    ///
    /// A switch is the whole window's: the app shows the session, the file panel is re-read from
    /// that session's worktree, and the file tabs start over — they name files in the tree that
    /// was just replaced.
    pub fn select(&mut self, index: usize) {
        if index < self.sessions.len() {
            self.active = index;
            if let Some(live) = self.live.as_mut() {
                live.switch(index);
            }
            // The app may have re-read this session's branch on the switch, so the row takes
            // whatever it now holds rather than the word it had when the list was built.
            if let Some(meta) = self.live.as_ref().and_then(|live| live.session(index)) {
                self.sessions[index] = meta;
            }
            self.reopen_files();
            self.reset_panes();
            self.refresh();
        }
    }

    /// Re-reads the file panel for the session now shown.
    ///
    /// A worktree that cannot be read leaves no panel rather than the previous session's rows:
    /// the session list already says the worktree is missing, and a stale tree would claim files
    /// that are not this session's. The editor the app handed over outlives the panel.
    pub(super) fn reopen_files(&mut self) {
        let Some(root) = self
            .sessions
            .get(self.active)
            .map(|session| session.worktree.clone())
        else {
            return;
        };
        let editor = self.files.as_ref().and_then(FilePanel::editor);
        let workspace = Workspace::open(&root).ok().map(|workspace| {
            let mut panel = FilePanel::new(&workspace);
            if let Some(editor) = editor.clone() {
                panel.set_editor(editor);
            }
            (workspace, panel)
        });
        match workspace {
            Some((workspace, panel)) => {
                self.workspace = Some(workspace);
                self.files = Some(panel);
            }
            None => {
                self.workspace = None;
                self.files = None;
            }
        }
    }

    /// Puts `text` in the composer, as typing would.
    pub fn set_prompt(&mut self, text: impl Into<String>) {
        self.prompt = TextEdit::default();
        self.prompt.text = text.into();
    }

    /// Replaces tab `index`'s frame with the newest one its thread published. An index past the
    /// end is ignored: a frame for a closed tab is not a reason to open one.
    pub fn set_frame(&mut self, index: usize, frame: perch_term::Frame) {
        if let Some(tab) = self.terminals.get_mut(index) {
            tab.frame = Some(frame);
        }
    }

    /// Shows the terminal tab at `index`. An index past the end is ignored, for the same reason a
    /// session index is: it names a tab that is gone.
    pub fn select_terminal(&mut self, index: usize) {
        if index < self.terminals.len() {
            self.terminal = index;
        }
    }

    /// Spawns a shell for a new terminal tab, in the active session's worktree, and shows it.
    ///
    /// This blocks until the emulator exists, so it is called when a tab is opened and never from
    /// a frame.
    pub fn open_terminal(&mut self) -> std::result::Result<usize, String> {
        let Some(session) = self.sessions.get(self.active) else {
            return Err("no session to open a shell for".to_string());
        };
        let cwd = session.worktree.clone();
        let title = cwd.file_name().map_or_else(
            || cwd.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        let (cols, rows) = self.grid.get();
        let spawned = TermSession::spawn(&cwd, cols, rows);
        let (handle, frames) = match spawned {
            Ok(spawned) => spawned,
            Err(error) => {
                // The dock says why there is no shell, in words, rather than showing an empty
                // grid as if one were starting.
                let words = format!("no shell: {error}");
                self.terminals
                    .push(TerminalTab::failed(title, words.clone()));
                self.terminal = self.terminals.len() - 1;
                return Err(words);
            }
        };
        self.terminals
            .push(TerminalTab::live(title, handle, frames, (cols, rows)));
        self.terminal = self.terminals.len() - 1;
        Ok(self.terminal)
    }

    /// Sends the composer's text as a turn, and empties the box when it was sent.
    ///
    /// The app owns the transcript: a refusal is a turn the app refused, and the sentence saying
    /// why is in the transcript the app keeps, not one invented here. A line that names a command
    /// is taken by the same press and never reaches the model as a user message.
    pub fn submit(&mut self) -> bool {
        let text = self.prompt.text.trim().to_owned();
        if text.is_empty() {
            return false;
        }
        self.close_menu();
        if self.run_command(&text) {
            // A command that found nothing to change leaves the box as it was sent, and is
            // emptied; `/skill` replaced the text with a prompt the person edits, which is kept.
            if self.prompt.text.trim() == text {
                self.prompt = TextEdit::default();
            }
            self.refresh();
            return true;
        }
        let Some(live) = self.live.as_mut() else {
            return false;
        };
        let sent = live.prompt(text);
        self.refresh();
        if sent.is_ok() {
            self.prompt = TextEdit::default();
        }
        true
    }

    /// Stops the turn, settling whatever it is waiting on as a refusal first.
    pub fn interrupt(&mut self) {
        if let Some(live) = self.live.as_mut() {
            live.cancel();
        }
        self.refresh();
    }

    /// Allows the call that is waiting.
    pub fn approve(&mut self) {
        if let Some(live) = self.live.as_mut() {
            live.approve();
        }
        self.refresh();
    }

    /// Refuses the call that is waiting.
    pub fn deny(&mut self) {
        if let Some(live) = self.live.as_mut() {
            live.deny();
        }
        self.refresh();
    }
}
