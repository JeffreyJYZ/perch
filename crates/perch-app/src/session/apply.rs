//! Folding one posted event into a session's transcript and its log.
//!
//! The dispatch thread posts events and this is the only code that applies them: the thread that
//! draws is the thread that mutates, so a turn's arrival can never race a frame. A card is added
//! from the call, before the call runs, so the column shows what is being decided rather than
//! only what came back.

use perch_agent::AgentEvent;
use perch_store::SessionEvent;

use super::SessionRuntime;
use crate::bridge::BridgeEvent;
use crate::turn::{Pending, question_text, reason_word};

impl SessionRuntime {
    /// Folds one posted event into the transcript and the log, and reports whether the session
    /// is now waiting on the user.
    pub(crate) fn apply(&mut self, event: BridgeEvent) -> bool {
        match event {
            // A notice changes nothing the turn is waiting on, so it is not a wait: the arm
            // still has to answer the function's question, which is why it says so explicitly.
            BridgeEvent::Notice(words) => {
                self.transcript.note(words);
                return false;
            }
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
                // The transcript's own ending is the one place the column says a turn was
                // cancelled: it notes the word and marks the open reply, so nothing here repeats
                // it as text. The log keeps `reason_word` for the same reason.
                self.transcript.apply(&AgentEvent::TurnEnd {
                    reason: reason.clone(),
                    usage,
                });
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
}
