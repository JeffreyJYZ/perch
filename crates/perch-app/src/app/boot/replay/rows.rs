//! Folding one replayed exchange into the column: the same events a live turn posts, in order.

use perch_agent::{AgentEvent, EndReason, ToolCall, Usage};
use perch_store::SessionEvent;
use perch_ui::Transcript;

use crate::session::CANCELLED;

/// Folds one exchange — a person's turn and everything the model answered — into `transcript`.
///
/// The events go through the same fold a live turn uses, so a reopened column shows what it showed
/// at the time: a reply, its cards with their status and output, the turn's closing summary, and
/// the word `cancelled` on a turn that was stopped rather than finished.
pub(super) fn fold(transcript: &mut Transcript, exchange: &[SessionEvent]) {
    for event in exchange {
        match event {
            SessionEvent::User { text } => transcript.push_user(text.clone()),
            SessionEvent::Assistant { text } => {
                transcript.apply(&AgentEvent::TextDelta(text.clone()));
            }
            SessionEvent::ToolCall { id, name, args } => {
                transcript.apply(&AgentEvent::ToolCall(ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    args: args.clone(),
                }));
            }
            SessionEvent::ToolResult { id, ok, text } => {
                transcript.apply(&AgentEvent::ToolResult {
                    id: id.clone(),
                    ok: *ok,
                    text: text.clone(),
                });
            }
            SessionEvent::TurnEnd {
                reason,
                tokens_in,
                tokens_out,
            } => {
                transcript.apply(&AgentEvent::TurnEnd {
                    reason: ending(reason),
                    usage: Usage {
                        input: *tokens_in,
                        output: *tokens_out,
                    },
                });
            }
            // A note is the log's own bookkeeping — a title that failed, a session removed — and
            // never part of the conversation, so the column is not told about it again.
            SessionEvent::Note { .. } => {}
        }
    }
}

/// The reason a log line recorded, back as the reason a turn ends with.
///
/// [`reason_word`](crate::turn::reason_word) writes `done`, `cancelled` or `errored: why`. A
/// cancelled turn comes back cancelled — the same word the column showed live — and is never read
/// as a failure; anything else is the error it was.
fn ending(reason: &str) -> EndReason {
    match reason {
        "done" => EndReason::Done,
        CANCELLED => EndReason::Cancelled,
        other => EndReason::Errored(other.strip_prefix("errored: ").unwrap_or(other).to_string()),
    }
}
