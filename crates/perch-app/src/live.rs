//! The app as the window sees it: the two trait implementations, and nothing else in them.
//!
//! [`Live`] is what draws and drives a turn; [`Panels`] is the plugin column drawn from the
//! worker's last snapshot, so the window reads the frames and words instead of asking a plugin
//! while it draws.

use std::path::Path;
use std::time::Duration;

use perch_plugins::PanelEntry;
use perch_ui::Transcript;
use perch_ui::shell::{Live, Panels, Waiting};

use crate::{AppState, Pending};

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

    fn model(&self) -> Option<String> {
        AppState::model(self)
    }

    fn models(&mut self) -> std::result::Result<Vec<String>, String> {
        AppState::list_models(self)
    }

    fn choose_model(&mut self, id: String) -> std::result::Result<(), String> {
        AppState::choose_model(self, &id).map_err(|error| error.to_string())
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

/// The plugin column, over the worker's last snapshot: the frames and the words are built off the
/// frame thread, so the window reads them rather than asking a plugin while it draws.
impl Panels for AppState {
    /// The plugin panels, then the update offer: the column is the one region a person can read
    /// without a session open, and an update is exactly that kind of news.
    fn panels(&self) -> Vec<PanelEntry> {
        let mut panels = self.snapshot.panels.clone();
        if let Some(status) = self.update_status.as_ref() {
            panels.push(status.panel());
        }
        panels
    }

    fn plugin_words(&self) -> Vec<String> {
        self.snapshot.words.clone()
    }

    /// A press on a panel control: refused here when the drawn panel cannot address it, and
    /// otherwise queued for the worker that owns the plugin host. The panel it changes is re-read
    /// into the next snapshot, which is what draws the change.
    fn panel_action(
        &mut self,
        plugin: &str,
        control: &str,
        value: Option<String>,
    ) -> std::result::Result<(), String> {
        AppState::panel_action(self, plugin, control, value)
    }
}
