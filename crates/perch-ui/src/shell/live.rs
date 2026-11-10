//! What the window asks of the app behind it, and the window that has none.

use std::time::Duration;

use crate::transcript::Transcript;

/// What one frame may spend folding a turn's events. The rest of the frame is the frame's: a
/// drawing thread that waits on a turn is a window that stops responding.
pub const BUDGET: Duration = Duration::from_millis(1);
/// How long the window waits before polling again while a turn is streaming.
pub const TURN_TICK: Duration = Duration::from_millis(8);
/// The same for a live shell, which has no channel to wake the window with: it is polled on a
/// tick because its output arrives whether or not anyone asked.
pub const SHELL_TICK: Duration = Duration::from_millis(16);

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

    /// Refuses it.
    fn deny(&mut self);

    /// Answers the waiting question with the option at `index`, counted from zero.
    fn pick(&mut self, index: usize);

    /// Shows the session at `index`.
    fn switch(&mut self, index: usize);
}

/// A window with no app behind it: the layout, the file panel and the dock alone.
///
/// This is what a `--tree` dump and a layout test run against: no data directory, no keychain.
pub struct Idle;

impl Live for Idle {
    fn pump(&mut self, _: Duration) -> bool {
        false
    }
    fn transcript(&self) -> Option<&Transcript> {
        None
    }
    fn waiting(&self) -> Waiting {
        Waiting::Nothing
    }
    fn busy(&self) -> bool {
        false
    }
    fn prompt(&mut self, _: String) -> std::result::Result<(), String> {
        Err("no session is open".to_string())
    }
    fn cancel(&mut self) {}
    fn approve(&mut self) {}
    fn deny(&mut self) {}
    fn pick(&mut self, _: usize) {}
    fn switch(&mut self, _: usize) {}
}

/// What the active session is waiting on, as the key map needs it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Waiting {
    /// Nothing: a turn is running, or none has started.
    #[default]
    Nothing,
    /// A call that needs the user's decision.
    Approval,
    /// The model's question, with `options` choices.
    Question {
        /// How many options the question offered.
        options: usize,
    },
}

impl Waiting {
    /// The words the window says while this is what it waits on.
    pub fn words(self) -> &'static str {
        match self {
            Waiting::Nothing => "",
            Waiting::Approval => "approval needed",
            Waiting::Question { .. } => "answer needed",
        }
    }
}
