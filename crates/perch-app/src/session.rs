//! One session's runtime: its transcript, its log, its agent, and what it is waiting on.
//!
//! A session folds what its dispatch thread posted into the transcript through [`apply`], so the
//! thread that draws is the thread that mutates and a turn's arrival can never race a frame.
//! Everything a turn needs from a person blocks that dispatch thread and nothing else.
//!
//! [`apply`]: SessionRuntime::apply

use std::sync::{Arc, Mutex};

use perch_agent::{AgentHandle, Dispatcher, Endpoint, Model};
use perch_store::{SessionEvent, SessionLog, SessionMeta};
use perch_ui::Transcript;

use crate::bridge::Bridge;
use crate::turn::Pending;

mod apply;

/// What the transcript says when a turn is cancelled.
///
/// A word, like every other state the window shows: the turn stopped, and saying so is the only
/// way the column is not silently missing an answer.
pub const CANCELLED: &str = "cancelled";

/// One session: its log, its agent, its transcript, and the thread that runs its tools.
pub struct SessionRuntime {
    pub meta: SessionMeta,
    pub transcript: Transcript,
    pub log: SessionLog,
    pub agent: AgentHandle,
    /// The model this session asks for on its next request, shared with the agent: changing it
    /// here changes the next turn and never the one streaming.
    pub model: Model,
    /// The address and key that request uses, shared for the same reason: a settings save reaches
    /// the session already on screen.
    pub endpoint: Endpoint,
    /// What remembers an "allow for session". One per session, because allowing `bash` here must
    /// not silently allow it in another; held so the allowances outlive the dispatch thread.
    #[allow(
        dead_code,
        reason = "held for the session's lifetime, read through the dispatch thread"
    )]
    dispatcher: Arc<Mutex<Dispatcher>>,
    pub(crate) bridge: Bridge,
    pending: Option<Pending>,
    /// Whether a turn is in flight, which includes waiting on the user: nothing can arrive until
    /// it is answered, so the window stops polling for it.
    running: bool,
    turns: u32,
    /// The text this turn has streamed, so the log keeps the reply the transcript showed.
    open_text: String,
    /// Messages sent while a turn was running: they go out, in order, as soon as it ends. A
    /// prompt that arrives mid-turn is queued, never interleaved into the turn in flight.
    queued: Vec<String>,
    /// The tokens the last turn reported, which is what the meter counts.
    ///
    /// `None` before the first turn: a fresh session has spent nothing, and drawing `0%` would
    /// claim a measurement nobody made.
    tokens: Option<u32>,
    /// Whether a compaction is on its way. Requests travel the agent's own queue, so one sent
    /// mid-turn lands right after that turn and before any prompt queued behind it.
    compacting: bool,
}

impl SessionRuntime {
    /// A session that has not run a turn yet: its log, its agent and the bridge it answers with.
    #[allow(
        clippy::too_many_arguments,
        reason = "the session's whole state, built once"
    )]
    pub(crate) fn new(
        meta: SessionMeta,
        transcript: Transcript,
        log: SessionLog,
        agent: AgentHandle,
        dispatcher: Arc<Mutex<Dispatcher>>,
        bridge: Bridge,
        model: Model,
        endpoint: Endpoint,
    ) -> SessionRuntime {
        SessionRuntime {
            meta,
            transcript,
            log,
            agent,
            model,
            endpoint,
            dispatcher,
            bridge,
            pending: None,
            running: false,
            turns: 0,
            open_text: String::new(),
            queued: Vec::new(),
            tokens: None,
            compacting: false,
        }
    }

    /// What this session is waiting for, when it is waiting.
    pub fn pending(&self) -> Option<&Pending> {
        self.pending.as_ref()
    }

    /// Whether the session is waiting for the user before it can go on.
    pub fn is_waiting(&self) -> bool {
        self.pending.is_some()
    }

    /// Whether a turn is in flight.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// How many turns have ended. One is the promise; a count makes a second one visible.
    pub fn turns_ended(&self) -> u32 {
        self.turns
    }

    /// The tokens the last turn reported, when a turn has ended.
    pub fn tokens_used(&self) -> Option<u32> {
        self.tokens
    }

    /// Marks a compaction as asked for, and answers false when one already was.
    pub(crate) fn begin_compact(&mut self) -> bool {
        let was = self.compacting;
        self.compacting = true;
        !was
    }

    /// Forgets that a compaction was asked for, because it answered.
    pub(crate) fn compacted(&mut self) {
        self.compacting = false;
    }

    /// This session's transcript, which the shell draws.
    pub fn transcript(&self) -> &Transcript {
        &self.transcript
    }

    /// Marks a turn as started and records the question the person asked.
    pub(crate) fn begin(&mut self, text: &str) {
        self.open_text.clear();
        self.running = true;
        self.transcript.push_user(text.to_string());
        let _ = self.log.append(&SessionEvent::User {
            text: text.to_string(),
        });
    }

    /// Whether a turn is in flight, which is what makes the next prompt wait its turn.
    pub(crate) fn running(&self) -> bool {
        self.running
    }

    /// Holds `text` until the turn in flight ends.
    pub(crate) fn queue(&mut self, text: String) {
        self.queued.push(text);
    }

    /// Starts the oldest queued message, if one is waiting. Answers whether it did.
    pub(crate) fn start_queued(&mut self) -> bool {
        if self.running {
            return false;
        }
        let Some(next) = self.queued.first().cloned() else {
            return false;
        };
        self.queued.remove(0);
        self.begin(&next);
        self.agent.prompt(next);
        true
    }

    /// Forgets whatever the session was waiting on, because it was answered.
    pub(crate) fn answered(&mut self) {
        self.pending = None;
    }
}
