//! What the window asks of the app behind it, and the window that has none.

use std::path::Path;
use std::time::Duration;

use perch_agent::ListedModel;
use perch_store::SessionMeta;

mod tick;

use crate::settings::SettingsDraft;
use crate::transcript::Transcript;
pub use tick::{BUDGET, SHELL_TICK, TURN_TICK};

/// What the window asks of the app behind it.
///
/// One trait, because there is one app: the shell never reaches past it for state, and a test can
/// stand [`Idle`] in its place. Every method is called on the drawing thread.
pub trait Live: 'static {
    /// Folds what the turn has posted into the app's transcript, waiting at most `budget`, and
    /// reports whether anything changed.
    fn pump(&mut self, budget: Duration) -> bool;

    /// The active session's transcript, when a session is open.
    fn transcript(&self) -> Option<&Transcript>;

    /// The transcript a press may change. The window draws a copy, so a change to the copy alone
    /// lasts one frame; `None` is no app, or no session to change.
    fn transcript_mut(&mut self) -> Option<&mut Transcript> {
        None
    }

    /// Opens a session on `worktree` and answers with its index and header, so the window can add
    /// the row without reading the directory again. The error is why it did not open, in words.
    fn new_session(
        &mut self,
        _worktree: &Path,
    ) -> std::result::Result<(usize, SessionMeta), String> {
        Err("no app is running, so no session was opened".to_string())
    }

    /// Forks the session at `index` into one of its own; answers its index and header.
    fn fork_session(&mut self, _index: usize) -> std::result::Result<(usize, SessionMeta), String> {
        Err("no app is running, so no session was forked".to_string())
    }

    /// Opens a session on the folder a person picked in the window, as that folder: the window
    /// asks for nothing about where it is, because choosing it was the decision.
    fn open_folder(&mut self, _path: &Path) -> std::result::Result<(usize, SessionMeta), String> {
        Err("no app is running, so no folder was opened".to_string())
    }

    /// What the active session is waiting on.
    fn waiting(&self) -> Waiting;

    /// Whether a turn is in flight, so the window knows to keep polling.
    fn busy(&self) -> bool;

    /// Sends `text` as a turn. The error is why it was not sent, in words.
    fn prompt(&mut self, text: String) -> std::result::Result<(), String>;

    /// Stops the turn: whatever is being waited on is settled first, so no call runs on the
    /// strength of an answer that was never given.
    fn cancel(&mut self);

    /// Allows the call that is waiting.
    fn approve(&mut self);

    /// Allows it, and stops the same tool asking again for the rest of the session.
    fn allow_for_session(&mut self) {}

    /// Refuses it.
    fn deny(&mut self);

    /// Answers the waiting question with the option at `index`, counted from zero.
    fn pick(&mut self, index: usize);

    /// Answers the waiting question with several options, in the order they were picked.
    fn pick_many(&mut self, _picks: &[usize]) {}

    /// Answers the waiting question with text the person typed.
    fn answer_text(&mut self, _text: String) {}

    /// Shows the session at `index`.
    fn switch(&mut self, index: usize);

    /// The header of the session at `index`, when one is open.
    ///
    /// The window keeps its own copy of the list, so a branch the app re-read after a checkout
    /// (or a title a rename wrote) reaches the row through this, never through a guess.
    fn session(&self, _index: usize) -> Option<SessionMeta> {
        None
    }

    /// The saved settings, when an app is behind the window. `None` is a window with no data
    /// directory, which the settings panel opens over with its defaults.
    fn settings(&self) -> Option<SettingsSnapshot> {
        None
    }

    /// The active session's model id, exactly as the composer shows it.
    fn model(&self) -> Option<String> {
        None
    }

    /// The provider.rs model ids, sorted as returned; the error is why nothing was listed. This is
    /// the blocking form — a click path uses [`Live::start_models`] and [`Live::models_ready`].
    fn models(&mut self) -> std::result::Result<Vec<ListedModel>, String> {
        Err("no app is running, so no models were listed".to_string())
    }

    /// Starts asking the provider what it offers, and returns at once.
    fn start_models(&mut self) {}

    /// The answer to [`Live::start_models`], once it is there, so a frame never waits on it.
    fn models_ready(&mut self) -> Option<std::result::Result<Vec<ListedModel>, String>> {
        None
    }

    /// The context window the provider states for this session's model, when it states one.
    fn model_limit(&self) -> Option<u32> {
        None
    }

    /// The tokens the last turn reported, when one has ended.
    fn tokens_used(&self) -> Option<u32> {
        None
    }

    /// The dollars the last turn cost, when the provider stated the model's price.
    ///
    /// A window with no price states no money at all: an invented bill is worse than no bill, so
    /// the meter leaves the amount out rather than showing a zero.
    fn turn_cost(&self) -> Option<f64> {
        None
    }

    /// The active session's todo list, from the table `todowrite` writes.
    ///
    /// Read on every frame rather than kept by the window: a `todowrite` inside a turn changes it
    /// mid-run, and a copy taken once would draw the plan the model has already moved past.
    fn todos(&self) -> Vec<perch_tools::Todo> {
        Vec::new()
    }

    /// Asks the session to compact its conversation, and returns at once: mid-turn the request
    /// runs at the end of the current turn, ahead of any prompt queued behind it.
    fn compact(&mut self) {}

    /// Picks `id` for the active session only: its next turn asks for it, no session restarts.
    fn choose_model(&mut self, _id: String) -> std::result::Result<(), String> {
        Err("no session is open, so the model was not changed".to_string())
    }

    /// Names the session at `index`, and answers its header as the list now carries it.
    ///
    /// The title is written where the next start reads it, so a rename outlives the window. An
    /// app that cannot write the log says why rather than answering a header it did not save.
    fn rename_session(
        &mut self,
        _index: usize,
        _title: &str,
    ) -> std::result::Result<SessionMeta, String> {
        Err("no app is running, so the title was not saved".to_string())
    }

    /// Deletes the session at `index`, stopping its bridge and dropping its runtime, and answers
    /// the index the window should show afterwards. The error is why nothing was deleted.
    fn delete_session(&mut self, _index: usize) -> std::result::Result<usize, String> {
        Err("no app is running, so no session was deleted".to_string())
    }

    /// Saves the theme a restart reads, and keeps it for the sessions already open.
    fn set_theme(&mut self, _theme: &str) -> std::result::Result<(), String> {
        Err("no app is running, so the theme was not saved".to_string())
    }

    /// The directory a session's export is written under, when the app has one.
    fn data_dir(&self) -> Option<std::path::PathBuf> {
        None
    }

    /// Writes `draft` and reports whether a key is stored afterwards.
    ///
    /// The error is why it was not written, in words. Validation belongs to the panel, so a
    /// caller here has already refused anything the person could see was wrong.
    fn save_settings(&mut self, _draft: SettingsDraft) -> std::result::Result<KeyStatus, String> {
        Err("no app is running, so nothing was saved".to_string())
    }
}

/// A window with no app behind it: the layout, the file panel and the dock alone.
///
/// This is what a `--tree` dump and a layout test run against: no data directory, no keychain.
pub use super::idle::Idle;

pub use super::settings_snapshot::{KeyStatus, SettingsSnapshot};

/// What the active session is waiting on, as the key map needs it.
pub use super::waiting::Waiting;
