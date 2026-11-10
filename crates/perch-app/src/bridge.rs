//! The seam between a turn's threads and the window.
//!
//! The agent streams on its own thread and the tool dispatcher runs on another; the window draws
//! on the main thread, which must never wait for either. Nothing here touches the transcript:
//! the dispatch thread posts a [`BridgeEvent`] for everything it needs shown and blocks on the one
//! channel that carries the user's answer, and the main thread folds what it receives into the
//! transcript when it next pumps.
//!
//! This is also where a tool result turns back into a message: the turn asked for the call and is
//! waiting, so every call is answered on the agent's handle — approved, denied or failed — and the
//! turn can ask the model again with the results in front of it. A cancel that lands before a call
//! starts is obeyed instead: the card is shown, and nothing runs.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use perch_agent::{
    AgentEvent, AgentHandle, Answer, Decision, Dispatcher, EndReason, ToolCall, Usage,
};
use perch_tools::{Preview, ToolCtx, ToolRegistry};

use crate::answer::Answers;

pub use crate::answer::ANSWER_TIMEOUT;

/// One thing the dispatch thread needs the window to show.
#[derive(Debug)]
pub enum BridgeEvent {
    /// Assistant text, appended to the open message.
    Text(String),
    /// A call the model finished asking for. The card is added from here, before the call runs.
    ToolCall(ToolCall),
    /// What the call did, once the dispatcher has decided and run it.
    ToolResult { id: String, ok: bool, text: String },
    /// The call named `name` needs the user's answer, and shows `preview` while it waits.
    Approval {
        id: String,
        name: String,
        preview: Option<Preview>,
    },
    /// The model asked the user something. `options` is empty for a free-text question.
    Question {
        prompt: String,
        options: Vec<String>,
        multiple: bool,
    },
    /// The turn is over; there is exactly one of these per turn.
    TurnEnd { reason: EndReason, usage: Usage },
}

/// The answer to whatever a session is waiting on.
///
/// One enum because there is one channel: a session is never waiting for an approval and a
/// question at the same time, so the reply needs no tag of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    Decision(Decision),
    Answer(Answer),
}

impl From<Decision> for Reply {
    fn from(decision: Decision) -> Reply {
        Reply::Decision(decision)
    }
}

impl From<Answer> for Reply {
    fn from(answer: Answer) -> Reply {
        Reply::Answer(answer)
    }
}

/// The window's side of one session: what the turn needs shown, and the way back in.
pub struct Bridge {
    /// Everything the dispatch thread posted, in order.
    pub events: Receiver<BridgeEvent>,
    replies: Sender<Reply>,
}

impl Bridge {
    /// Answers whatever the session is waiting on. False when nobody is waiting any more.
    pub fn answer(&self, reply: Reply) -> bool {
        self.replies.send(reply).is_ok()
    }
}

/// Starts the dispatch thread for one session and hands back the window's side.
///
/// The dispatcher is shared rather than moved so the session owns it — it is what remembers an
/// "allow for session" — while the thread that runs calls is the only one that locks it. The
/// handle is shared for the same reason the dispatcher is: it is the conversation's one way in,
/// so the results of a round reach the turn that asked for them.
pub fn spawn(
    agent: AgentHandle,
    events: Receiver<AgentEvent>,
    registry: &Arc<ToolRegistry>,
    ctx: ToolCtx,
    dispatcher: &Arc<Mutex<Dispatcher>>,
) -> Bridge {
    let registry = Arc::clone(registry);
    let dispatcher = Arc::clone(dispatcher);
    let (out, incoming) = mpsc::channel();
    let (replies, incoming_replies) = mpsc::channel();
    std::thread::Builder::new()
        .name("perch-dispatch".to_string())
        .spawn(move || {
            run(
                &agent,
                events,
                &out,
                &incoming_replies,
                &registry,
                &ctx,
                &dispatcher,
            )
        })
        .expect("spawn the dispatch thread");
    Bridge {
        events: incoming,
        replies,
    }
}

/// Reads the turn's events and runs the calls it asks for, in order, one call at a time.
///
/// Every event is forwarded, including a `TurnEnd` that arrives while a call is being answered:
/// the window learns the turn is over, and the dispatch loop then ends with the channel.
fn run(
    agent: &AgentHandle,
    events: Receiver<AgentEvent>,
    out: &Sender<BridgeEvent>,
    replies: &Receiver<Reply>,
    registry: &ToolRegistry,
    ctx: &ToolCtx,
    dispatcher: &Mutex<Dispatcher>,
) {
    for event in events {
        let posted = match event {
            AgentEvent::TextDelta(text) => out.send(BridgeEvent::Text(text)).is_ok(),
            AgentEvent::ToolResult { id, ok, text } => {
                out.send(BridgeEvent::ToolResult { id, ok, text }).is_ok()
            }
            AgentEvent::ToolCall(call) => {
                dispatch(call, agent, out, replies, registry, ctx, dispatcher)
            }
            AgentEvent::TurnEnd { reason, usage } => {
                out.send(BridgeEvent::TurnEnd { reason, usage }).is_ok()
            }
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
    registry: &ToolRegistry,
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
    let mut dispatcher = lock(dispatcher);
    let replies = dispatcher.run(std::slice::from_ref(&call), registry, ctx, &mut interactor);
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
        if !agent.reply(reply) {
            return false;
        }
    }
    true
}

fn lock(dispatcher: &Mutex<Dispatcher>) -> std::sync::MutexGuard<'_, Dispatcher> {
    dispatcher
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
