//! One session's runtime: its transcript, its log, its agent, and what it is waiting on.
//!
//! A session folds what its dispatch thread posted into the transcript through [`apply`], so the
//! thread that draws is the thread that mutates and a turn's arrival can never race a frame.
//! Everything a turn needs from a person blocks that dispatch thread and nothing else.
//!
//! [`apply`]: SessionRuntime::apply

use std::sync::{Arc, Mutex};

use perch_agent::{AgentEvent, AgentHandle, Dispatcher, EndReason, Endpoint, Model};
use perch_store::{SessionEvent, SessionLog, SessionMeta};
use perch_ui::Transcript;

use crate::bridge::{Bridge, BridgeEvent};
use crate::turn::{Pending, question_text, reason_word};

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
    /// The model this session asks for on its next request. Shared with the agent, so changing it
    /// here changes the next turn and never the one streaming.
    pub model: Model,
    /// The address and key this session's next request uses, shared with the agent for the same
    /// reason: a settings save must reach the session already on screen.
    pub endpoint: Endpoint,
    /// What remembers an "allow for session". One per session, because allowing `bash` here
    /// must not silently allow it in another. Held so the same allowances are reachable without a
    /// second dispatcher appearing.
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
}

impl SessionRuntime {
    /// A session that has not run a turn yet: its log, its agent and the bridge it answers with.
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

    /// This session's transcript, which the shell draws.
    pub fn transcript(&self) -> &Transcript {
        &self.transcript
    }

    /// Folds one posted event into the transcript and the log, and reports whether the session
    /// is now waiting on the user.
    pub(crate) fn apply(&mut self, event: BridgeEvent) -> bool {
        match event {
            BridgeEvent::Text(text) => {
                self.transcript.apply(&AgentEvent::TextDelta(text.clone()));
                self.open_text.push_str(&text);
            }
            BridgeEvent::ToolCall(call) => {
                self.transcript.apply(&AgentEvent::ToolCall(call.clone()));
                // The log is the conversation's only durable copy; a write failing is worth
                // dropping rather than stopping the turn the user is watching.
                let _ = self.log.append(&SessionEvent::ToolCall {
                    id: call.id,
                    name: call.name,
                    args: call.args,
                });
            }
            BridgeEvent::ToolResult { id, ok, text } => {
                self.transcript.apply(&AgentEvent::ToolResult {
                    id: id.clone(),
                    ok,
                    text: text.clone(),
                });
                let _ = self.log.append(&SessionEvent::ToolResult { id, ok, text });
            }
            BridgeEvent::Approval { id, name, preview } => {
                // A tool that asks and shows nothing keeps its card saying "running": the column
                // has one approval state and it is the preview's. The wait below is what holds
                // the turn either way.
                if let Some(preview) = preview {
                    self.transcript.request_approval(&id, preview);
                }
                self.pending = Some(Pending::Approval { id, name });
                return true;
            }
            BridgeEvent::Question {
                prompt,
                options,
                multiple,
            } => {
                self.transcript
                    .apply(&AgentEvent::TextDelta(question_text(&prompt, &options)));
                self.pending = Some(Pending::Question {
                    prompt,
                    options,
                    multiple,
                });
                return true;
            }
            BridgeEvent::TurnEnd { reason, usage } => {
                self.transcript.apply(&AgentEvent::TurnEnd {
                    reason: reason.clone(),
                    usage,
                });
                if reason == EndReason::Cancelled {
                    // The column has no note item and this lane does not own its enum, so the
                    // word takes the same path a reply's text does — one item, saying the turn
                    // stopped. When the transcript gains a note item this is one line to change.
                    self.transcript
                        .apply(&AgentEvent::TextDelta(CANCELLED.into()));
                }
                let reply = std::mem::take(&mut self.open_text);
                if !reply.is_empty() {
                    let _ = self.log.append(&SessionEvent::Assistant { text: reply });
                }
                let _ = self.log.append(&SessionEvent::TurnEnd {
                    reason: reason_word(&reason),
                    tokens_in: usage.input,
                    tokens_out: usage.output,
                });
                self.pending = None;
                self.running = false;
                self.turns += 1;
            }
        }
        false
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

    /// Forgets whatever the session was waiting on, because it was answered.
    pub(crate) fn answered(&mut self) {
        self.pending = None;
    }
}
