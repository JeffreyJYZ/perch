//! The seam between a turn's threads and the window.
//!
//! The agent streams on its own thread and the tool dispatcher runs on another; the window draws
//! on the main thread, which must never wait for either. Nothing here touches the transcript:
//! the dispatch thread posts a [`BridgeEvent`] for everything it needs shown and blocks on the one
//! channel that carries the user's answer, and the main thread folds what it receives into the
//! transcript when it next pumps.
//!
//! The block is bounded. A question or an approval nobody answers gives up after
//! [`ANSWER_TIMEOUT`] and behaves as a refusal, so a closed window or a dropped session cannot
//! leave a tool running on the strength of an answer that never came.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use perch_agent::{
    AgentEvent, Answer, Decision, Dispatcher, EndReason, Interactor, ToolCall, Usage,
};
use perch_tools::{Preview, ToolCtx, ToolRegistry};

/// How long one call waits for the person in front of the window.
///
/// Long enough for someone to read a diff, short enough that an abandoned window does not keep a
/// thread alive forever. A timeout is a refusal, never an approval.
pub const ANSWER_TIMEOUT: Duration = Duration::from_secs(300);

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
/// "allow for session" — while the thread that runs calls is the only one that locks it.
pub fn spawn(
    events: Receiver<AgentEvent>,
    registry: Arc<ToolRegistry>,
    ctx: ToolCtx,
    dispatcher: Arc<Mutex<Dispatcher>>,
) -> Bridge {
    let (out, incoming) = mpsc::channel();
    let (replies, incoming_replies) = mpsc::channel();
    std::thread::Builder::new()
        .name("perch-dispatch".to_string())
        .spawn(move || {
            run(
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
fn run(
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
            AgentEvent::ToolCall(call) => dispatch(call, out, replies, registry, ctx, dispatcher),
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

/// Runs one call and reports its one result. False when the window stopped listening.
fn dispatch(
    call: ToolCall,
    out: &Sender<BridgeEvent>,
    replies: &Receiver<Reply>,
    registry: &ToolRegistry,
    ctx: &ToolCtx,
    dispatcher: &Mutex<Dispatcher>,
) -> bool {
    if out.send(BridgeEvent::ToolCall(call.clone())).is_err() {
        return false;
    }
    let mut interactor = BridgeInteractor {
        id: call.id.clone(),
        out,
        replies,
    };
    let mut dispatcher = lock(dispatcher);
    let replies = dispatcher.run(std::slice::from_ref(&call), registry, ctx, &mut interactor);
    for reply in replies {
        let text = reply.message.content.unwrap_or_default();
        if out
            .send(BridgeEvent::ToolResult {
                id: call.id.clone(),
                ok: reply.ok,
                text,
            })
            .is_err()
        {
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

/// The dispatcher's view of the person in front of the window: post what is needed, wait for the
/// answer, and give up as a refusal if it never comes.
struct BridgeInteractor<'a> {
    /// The call being decided, so the approval lands on the right card.
    id: String,
    out: &'a Sender<BridgeEvent>,
    replies: &'a Receiver<Reply>,
}

impl Interactor for BridgeInteractor<'_> {
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
