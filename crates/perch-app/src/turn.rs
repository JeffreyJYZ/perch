//! The turn-shaped vocabulary a session uses: what it is waiting on, what a pump did, and how an
//! event becomes a line of words.
//!
//! These are the parts of a session that are not its state — a session's transcript, log and agent
//! live in `session` — kept apart so a caller can name a wait without pulling in the thread.

use perch_agent::EndReason;

/// What one pump did, so a caller can tell a quiet frame from a moving one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pumped {
    /// Whether any event reached the transcript.
    pub changed: bool,
    /// Whether the session is now waiting for the user, which is where a caller stops.
    pub waiting: bool,
}

/// What a session is waiting on, and therefore what `AppState::answer` settles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pending {
    /// A call that needs the user's decision before it runs.
    Approval { id: String, name: String },
    /// The model's question.
    Question {
        prompt: String,
        options: Vec<String>,
        multiple: bool,
    },
}

/// The question and its options as the transcript shows them.
pub(crate) fn question_text(prompt: &str, options: &[String]) -> String {
    let mut text = prompt.to_string();
    for (index, option) in options.iter().enumerate() {
        text.push_str(&format!("\n{}. {option}", index + 1));
    }
    text
}

/// How a turn's ending is written in the log.
pub(crate) fn reason_word(reason: &EndReason) -> String {
    match reason {
        EndReason::Done => "done".to_string(),
        EndReason::Cancelled => crate::session::CANCELLED.to_string(),
        EndReason::Errored(why) => format!("errored: {why}"),
    }
}
