//! Starting, stopping and answering one turn.

use perch_agent::{Answer, Decision};

use super::boot::say;
use super::{AppState, Error, Result};
use crate::session::SessionRuntime;
use crate::turn::Pending;

impl AppState {
    /// Sends the composer's text as a turn: the question is recorded, logged, and handed to the
    /// agent. A session whose worktree has gone refuses it and says why in its own transcript.
    pub fn prompt(&mut self, text: impl Into<String>) -> Result<()> {
        let text = text.into();
        let Some(runtime) = self.sessions.get_mut(self.active) else {
            return Err(Error::NoSession);
        };
        if !runtime.meta.worktree.exists() {
            let error = Error::MissingWorktree(runtime.meta.worktree.clone());
            say(&mut runtime.transcript, &format!("not sent: {error}"));
            return Err(error);
        }
        runtime.begin(&text);
        runtime.agent.prompt(text);
        Ok(())
    }

    /// Stops the running turn. It still ends, once, and the transcript says how.
    pub fn cancel(&mut self) {
        if let Some(runtime) = self.sessions.get(self.active) {
            runtime.agent.cancel();
        }
    }

    /// Stops the turn and settles whatever it is waiting on first: a question is dismissed and an
    /// approval is refused, so no call runs on the strength of an answer nobody gave.
    pub fn interrupt(&mut self) {
        match self
            .sessions
            .get(self.active)
            .and_then(SessionRuntime::pending)
            .cloned()
        {
            Some(Pending::Approval { .. }) => {
                self.answer(Decision::Deny);
            }
            Some(Pending::Question { .. }) => {
                self.answer(Answer::Cancelled);
            }
            None => {}
        }
        self.cancel();
    }

    /// Whether the active session is waiting on an approval.
    fn approval_waiting(&self) -> bool {
        matches!(
            self.sessions
                .get(self.active)
                .and_then(SessionRuntime::pending),
            Some(Pending::Approval { .. })
        )
    }

    /// Allows the call that is waiting, and reports whether one was.
    pub fn allow(&mut self) -> bool {
        self.approval_waiting() && self.answer(Decision::Allow)
    }

    /// Refuses it, and reports whether one was waiting.
    pub fn refuse(&mut self) -> bool {
        self.approval_waiting() && self.answer(Decision::Deny)
    }

    /// Allows the call that is waiting for the rest of the session, so the same tool does not
    /// ask again, and reports whether one was waiting.
    pub fn allow_for_session(&mut self) -> bool {
        self.approval_waiting() && self.answer(Decision::AllowForSession)
    }

    /// Answers the model's question with the option at `index`, counted from zero, and reports
    /// whether a question with that option was waiting.
    pub fn choose(&mut self, index: usize) -> bool {
        let Some(Pending::Question { options, .. }) = self
            .sessions
            .get(self.active)
            .and_then(SessionRuntime::pending)
        else {
            return false;
        };
        index < options.len() && self.answer(Answer::Picks(vec![index]))
    }

    /// Answers the model's question with every option at the indexes given, and reports whether a
    /// question offering all of them was waiting. An empty set is refused rather than sent: the
    /// tool reads it as a dismissal, which is what [`Answer::Cancelled`] is for.
    pub fn choose_many(&mut self, indexes: &[usize]) -> bool {
        let offered = match self
            .sessions
            .get(self.active)
            .and_then(SessionRuntime::pending)
        {
            Some(Pending::Question { options, .. }) => options.len(),
            _ => return false,
        };
        if indexes.is_empty() || indexes.iter().any(|index| *index >= offered) {
            return false;
        }
        self.answer(Answer::Picks(indexes.to_vec()))
    }

    /// Answers the model's question with text the person typed, and reports whether a question
    /// was waiting.
    pub fn answer_text(&mut self, text: impl Into<String>) -> bool {
        matches!(
            self.sessions
                .get(self.active)
                .and_then(SessionRuntime::pending),
            Some(Pending::Question { .. })
        ) && self.answer(Answer::Text(text.into()))
    }

    /// Answers whatever the session is waiting on — an approval or a question — and reports
    /// whether anyone was waiting.
    pub fn answer(&mut self, reply: impl Into<crate::Reply>) -> bool {
        let Some(runtime) = self.sessions.get_mut(self.active) else {
            return false;
        };
        let sent = runtime.bridge.answer(reply.into());
        if sent {
            runtime.answered();
        }
        sent
    }
}
