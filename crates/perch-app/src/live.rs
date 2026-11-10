//! The app as the window sees it: one trait implementation, and nothing else in it.

use std::path::Path;
use std::time::Duration;

use perch_ui::Transcript;
use perch_ui::shell::{Live, Waiting};

use crate::app::{AppState, Pending};

impl Live for AppState {
    fn pump(&mut self, budget: Duration) -> bool {
        AppState::pump(self, budget).changed
    }

    fn transcript(&self) -> Option<&Transcript> {
        self.active_session().map(crate::SessionRuntime::transcript)
    }

    /// The session's own transcript, not the window's copy: a card opened here is opened for as
    /// long as the session lives, not for as long as one frame is drawn.
    fn transcript_mut(&mut self) -> Option<&mut Transcript> {
        self.active_session_mut()
            .map(|session| &mut session.transcript)
    }

    /// Starts a session on `worktree` and answers with the row the window appends.
    fn new_session(
        &mut self,
        worktree: &Path,
    ) -> std::result::Result<(usize, perch_store::SessionMeta), String> {
        let index = AppState::new_session(self, worktree).map_err(|error| error.to_string())?;
        let meta = self
            .active_session()
            .map(|session| session.meta.clone())
            .ok_or_else(|| "the session did not open".to_string())?;
        Ok((index, meta))
    }

    fn waiting(&self) -> Waiting {
        match self
            .active_session()
            .and_then(crate::SessionRuntime::pending)
        {
            Some(Pending::Approval { .. }) => Waiting::Approval,
            Some(Pending::Question { options, .. }) => Waiting::Question {
                options: options.len(),
            },
            None => Waiting::Nothing,
        }
    }

    fn busy(&self) -> bool {
        AppState::busy(self)
    }

    fn prompt(&mut self, text: String) -> std::result::Result<(), String> {
        AppState::prompt(self, text).map_err(|error| error.to_string())
    }

    fn cancel(&mut self) {
        AppState::interrupt(self);
    }

    fn approve(&mut self) {
        AppState::allow(self);
    }

    fn deny(&mut self) {
        AppState::refuse(self);
    }

    fn pick(&mut self, index: usize) {
        AppState::choose(self, index);
    }

    fn switch(&mut self, index: usize) {
        let _ = AppState::switch(self, index);
    }

    fn settings(&self) -> Option<perch_ui::shell::SettingsSnapshot> {
        Some(AppState::settings(self))
    }

    fn save_settings(
        &mut self,
        draft: perch_ui::SettingsDraft,
    ) -> std::result::Result<perch_ui::shell::KeyStatus, String> {
        AppState::save_settings(self, draft)
    }
}
