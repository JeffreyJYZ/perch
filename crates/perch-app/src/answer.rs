//! The window's answers to the questions a turn asks.
//!
//! The dispatcher asks through [`Interactor`]; this is the implementation whose answers come from
//! a person. It posts what it needs shown on the bridge and blocks on the one channel the window
//! answers on. The block is bounded, because a closed window or a dropped session must not leave
//! a tool waiting on an answer that will never come.

use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use perch_agent::{Answer, Decision, Interactor};
use perch_tools::Preview;

use crate::bridge::{BridgeEvent, Reply};

/// How long one call waits for the person in front of the window.
///
/// Long enough for someone to read a diff, short enough that an abandoned window does not keep a
/// thread alive forever. A timeout is a refusal, never an approval.
pub const ANSWER_TIMEOUT: Duration = Duration::from_secs(300);

/// The dispatcher's view of the person in front of the window: post what is needed, wait for the
/// answer, and give up as a refusal if it never comes.
pub struct Answers<'a> {
    /// The call being decided, so the approval lands on the right card.
    pub id: String,
    pub out: &'a Sender<BridgeEvent>,
    pub replies: &'a Receiver<Reply>,
}

impl Interactor for Answers<'_> {
    fn approve(&mut self, name: &str, preview: Option<&Preview>) -> Decision {
        let event = BridgeEvent::Approval {
            id: self.id.clone(),
            name: name.to_string(),
            preview: preview.cloned(),
        };
        if self.out.send(event).is_err() {
            return Decision::Deny;
        }
        match self.replies.recv_timeout(ANSWER_TIMEOUT) {
            Ok(Reply::Decision(decision)) => decision,
            // Nobody answered, or the window sent the reply for the other kind of wait: a call
            // that was not allowed does not run.
            _ => Decision::Deny,
        }
    }

    fn ask(&mut self, prompt: &str, options: &[String], multiple: bool) -> Answer {
        let event = BridgeEvent::Question {
            prompt: prompt.to_string(),
            options: options.to_vec(),
            multiple,
        };
        if self.out.send(event).is_err() {
            return Answer::Cancelled;
        }
        match self.replies.recv_timeout(ANSWER_TIMEOUT) {
            Ok(Reply::Answer(answer)) => answer,
            // A question nobody answers is a question the user dismissed, which the tool reports
            // rather than inventing a choice.
            _ => Answer::Cancelled,
        }
    }
}
