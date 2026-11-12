//! Waiting for the results of the calls one round asked for.
//!
//! The turn does not run tools; it asks for them and waits, so the reply channel is the other half
//! of the conversation. Every call the model made must be answered before the next request goes
//! out: the provider addresses each result to a call by its id, and a conversation with a call and
//! no result is rejected. A denial is answered too — "the user denied it" is a fact the model can
//! act on, while silence is not.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use crate::dispatch::ToolReply;
use crate::event::ToolCall;
use crate::provider::Message;

/// How often a wait re-checks the cancel counter.
///
/// Short enough that a cancel is felt while a tool is still running, long enough that waiting
/// costs a sleeping thread rather than a spinning one. There is no overall deadline: the window
/// bounds its own answer at `ANSWER_TIMEOUT`, and a tool that takes minutes is a slow tool, not a
/// stuck turn.
const POLL: Duration = Duration::from_millis(25);

/// Why a round stopped waiting before every call was answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lost {
    /// The user cancelled the turn.
    Cancelled,
    /// The tool runner went away without answering every call.
    Gone,
}

/// Waits for one reply per call and returns the tool messages, in the order the model asked.
///
/// Replies are matched by the call id the provider gave, so a result that arrives for a call this
/// round never made — a leftover from a turn that was cancelled while its tool was still running —
/// is dropped instead of answering this conversation with another one's result.
pub fn collect(
    replies: &Receiver<ToolReply>,
    calls: &[ToolCall],
    stopped: &dyn Fn() -> bool,
) -> Result<Vec<Message>, Lost> {
    let mut waiting: BTreeSet<String> = calls.iter().map(|call| call.id.clone()).collect();
    let mut answered: BTreeMap<String, Message> = BTreeMap::new();
    while !waiting.is_empty() {
        if stopped() {
            return Err(Lost::Cancelled);
        }
        match replies.recv_timeout(POLL) {
            Ok(reply) => {
                let Some(id) = reply.message.tool_call_id.clone() else {
                    continue;
                };
                if waiting.remove(&id) {
                    answered.insert(id, reply.message);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Err(Lost::Gone),
        }
    }
    // The last reply can arrive in the same breath as a cancel, and then the wait set empties
    // without the loop ever reaching the check above. A cancelled turn must not go on to ask the
    // model again with results the user has stopped waiting for, so the check is repeated here.
    if stopped() {
        return Err(Lost::Cancelled);
    }
    Ok(calls
        .iter()
        .filter_map(|call| answered.remove(&call.id))
        .collect())
}
