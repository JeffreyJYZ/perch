//! Reading a turn's events and running the calls it asks for, one at a time.

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Mutex, MutexGuard};

use perch_agent::{AgentEvent, AgentHandle, Dispatcher, ToolCall};
use perch_tools::ToolCtx;

use super::{BridgeEvent, Reply};
use crate::answer::Answers;
use crate::live_registry::Live;

/// Reads the turn's events and runs the calls it asks for, in order, one call at a time.
///
/// Every event is forwarded, including a `TurnEnd` that arrives while a call is being answered:
/// the window learns the turn is over, and the dispatch loop then ends with the channel.
pub(super) fn run(
    agent: &AgentHandle,
    events: Receiver<AgentEvent>,
    out: &Sender<BridgeEvent>,
    replies: &Receiver<Reply>,
    live: &Live,
    ctx: &ToolCtx,
    dispatcher: &Mutex<Dispatcher>,
) {
    for event in events {
        let posted = match event {
            AgentEvent::TextDelta(text) => out.send(BridgeEvent::Text(text)).is_ok(),
            AgentEvent::ReasoningDelta(words) => out.send(BridgeEvent::Reasoning(words)).is_ok(),
            AgentEvent::ToolResult { id, ok, text } => {
                out.send(BridgeEvent::ToolResult { id, ok, text }).is_ok()
            }
            AgentEvent::ToolCall(call) => {
                dispatch(call, agent, out, replies, live, ctx, dispatcher)
            }
            AgentEvent::TurnEnd { reason, usage } => {
                out.send(BridgeEvent::TurnEnd { reason, usage }).is_ok()
            }
            // A compaction is not a reply and not a turn: the column says what happened in its
            // own voice, and a session waiting on a queued prompt learns it can go on.
            AgentEvent::Compacted { dropped, exchanges } => out
                .send(BridgeEvent::Compacted { dropped, exchanges })
                .is_ok(),
        };
        // A closed channel means the window is gone; nothing left to show a result to.
        if !posted {
            return;
        }
    }
}

/// Runs one call, shows its result, and answers the turn with it. False when the window stopped
/// listening.
fn dispatch(
    call: ToolCall,
    agent: &AgentHandle,
    out: &Sender<BridgeEvent>,
    replies: &Receiver<Reply>,
    live: &Live,
    ctx: &ToolCtx,
    dispatcher: &Mutex<Dispatcher>,
) -> bool {
    if out.send(BridgeEvent::ToolCall(call.clone())).is_err() {
        return false;
    }
    // The turn that asked for this call may already be cancelled: the card is shown, so the
    // column says what was asked, and the call does not run.
    if agent.watch().cancelled() {
        return true;
    }
    let mut interactor = Answers {
        id: call.id.clone(),
        out,
        replies,
    };
    // The registry as it is when this call starts. A reload that lands mid-turn is therefore
    // deferred to the next call, and never splits one call across two tool lists.
    let registry = live.load();
    let mut dispatcher = lock(dispatcher);
    let replies = dispatcher.run(std::slice::from_ref(&call), &registry, ctx, &mut interactor);
    for reply in replies {
        let ok = reply.ok;
        let text = reply.message.content.clone().unwrap_or_default();
        if out
            .send(BridgeEvent::ToolResult {
                id: call.id.clone(),
                ok,
                text,
            })
            .is_err()
        {
            return false;
        }
        // The model is waiting on this before it can answer, and a denied call is answered too:
        // "the user denied it" is something it can act on, silence is not.
        //
        // A cancel that landed while the call waited for its answer ends the turn, so the result
        // is shown and not handed on: replying would ask the model a question the user has
        // already stopped waiting for, which is a request nobody can cancel in time.
        if agent.watch().cancelled() {
            return true;
        }
        if !agent.reply(reply) {
            return false;
        }
    }
    true
}

fn lock(dispatcher: &Mutex<Dispatcher>) -> MutexGuard<'_, Dispatcher> {
    dispatcher
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
