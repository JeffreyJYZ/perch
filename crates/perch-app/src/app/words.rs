//! What the column says about the sources themselves: a plugin that would not load, a server that
//! would not start, a plugin whose guest trapped.
//!
//! The sources open on the worker, so these are known a frame or two after the window draws rather
//! than before it. [`AppState::say_new_words`] is where a late one reaches the sessions that were
//! already open; a session opened afterwards is given them at creation.

use perch_ui::Transcript;

use super::AppState;
use crate::session::SessionRuntime;

/// Says `words` in a transcript as one item, in the column's quieter voice.
///
/// A system line is not a reply: it goes through [`Transcript::note`], which closes any open
/// assistant item first, so a startup word can never be read as something the model wrote.
pub(super) fn say(transcript: &mut Transcript, words: &str) {
    transcript.note(words.to_string());
}

impl AppState {
    /// Says any word the sources produced since the last frame, in every open session.
    ///
    /// Each word is said once: a session opened later is given the words its snapshot already
    /// carries. A turn in flight holds them, because a note joining a streaming reply would read
    /// as something the model said.
    pub(crate) fn say_new_words(&mut self) {
        if self.sessions.iter().any(SessionRuntime::is_running) {
            return;
        }
        let fresh: Vec<String> = self
            .snapshot
            .words
            .iter()
            .filter(|word| !self.spoken.contains(word))
            .cloned()
            .collect();
        for word in fresh {
            for session in &mut self.sessions {
                say(&mut session.transcript, &word);
            }
            self.spoken.push(word);
        }
    }
}
