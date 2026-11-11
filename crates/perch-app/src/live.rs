//! The app as the window sees it: the two trait implementations, and nothing else in them.
//!
//! [`Live`] is what draws and drives a turn; [`Panels`] is the plugin column drawn from the
//! worker's last snapshot, so the window reads the frames and words instead of asking a plugin
//! while it draws.

use std::path::Path;
use std::time::Duration;

use perch_ui::Transcript;
use perch_ui::shell::{Live, Waiting};

use crate::{AppState, Pending};

mod panels;

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
        row(self, index)
    }

    /// Opens a session on a folder a person picked, accepted wherever it is.
    fn open_folder(
        &mut self,
        path: &Path,
    ) -> std::result::Result<(usize, perch_store::SessionMeta), String> {
        let index = AppState::open_folder(self, path).map_err(|error| error.to_string())?;
        row(self, index)
    }

    fn waiting(&self) -> Waiting {
        match self
            .active_session()
            .and_then(crate::SessionRuntime::pending)
        {
            Some(Pending::Approval { .. }) => Waiting::Approval,
            Some(Pending::Question {
                options, multiple, ..
            }) => Waiting::Question {
                options: options.len(),
                multiple: *multiple,
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

    fn allow_for_session(&mut self) {
        AppState::allow_for_session(self);
    }

    fn deny(&mut self) {
        AppState::refuse(self);
    }

    fn pick(&mut self, index: usize) {
        AppState::choose(self, index);
    }

    fn pick_many(&mut self, picks: &[usize]) {
        AppState::choose_many(self, picks);
    }

    fn answer_text(&mut self, text: String) {
        AppState::answer_text(self, text);
    }

    fn switch(&mut self, index: usize) {
        let _ = AppState::switch(self, index);
    }

    fn session(&self, index: usize) -> Option<perch_store::SessionMeta> {
        self.sessions.get(index).map(|session| session.meta.clone())
    }

    fn model(&self) -> Option<String> {
        AppState::model(self)
    }

    fn models(&mut self) -> std::result::Result<Vec<perch_agent::ListedModel>, String> {
        AppState::list_models(self)
    }

    fn start_models(&mut self) {
        AppState::start_models(self);
    }

    fn models_ready(
        &mut self,
    ) -> Option<std::result::Result<Vec<perch_agent::ListedModel>, String>> {
        AppState::models_ready(self)
    }

    fn model_limit(&self) -> Option<u32> {
        AppState::model_limit(self)
    }

    fn tokens_used(&self) -> Option<u32> {
        self.active_session()
            .and_then(crate::SessionRuntime::tokens_used)
    }

    fn todos(&self) -> Vec<perch_tools::Todo> {
        AppState::todos(self)
    }

    fn compact(&mut self) {
        let _ = AppState::compact(self);
    }

    fn choose_model(&mut self, id: String) -> std::result::Result<(), String> {
        AppState::choose_model(self, &id).map_err(|error| error.to_string())
    }

    fn rename_session(
        &mut self,
        index: usize,
        title: &str,
    ) -> std::result::Result<perch_store::SessionMeta, String> {
        AppState::rename_session(self, index, title).map_err(|error| error.to_string())
    }

    fn delete_session(&mut self, index: usize) -> std::result::Result<usize, String> {
        AppState::delete_session(self, index).map_err(|error| error.to_string())
    }

    fn set_theme(&mut self, theme: &str) -> std::result::Result<(), String> {
        AppState::save_theme(self, theme).map_err(|error| error.to_string())
    }

    fn data_dir(&self) -> Option<std::path::PathBuf> {
        Some(self.dir.clone())
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

/// The header of the session just opened: the row the window appends without reading the disk.
fn row(
    app: &AppState,
    index: usize,
) -> std::result::Result<(usize, perch_store::SessionMeta), String> {
    let meta = app
        .active_session()
        .map(|session| session.meta.clone())
        .ok_or_else(|| "the session did not open".to_string())?;
    Ok((index, meta))
}
