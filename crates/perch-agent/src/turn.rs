//! The agent loop's surface: one thread per agent, one prompt per turn.
//!
//! [`Agent::spawn`] starts the thread and [`AgentHandle`] is the only way to talk to it: a prompt
//! goes in, the results of the calls it asks for come back, and a cancel stops whatever is running.
//! What one turn actually does — request, stream, run its tools, ask again — is the rounds module.
//!
//! Cancellation is a monotonic counter rather than a flag: see [`Cancel`]. Each turn snapshots the
//! counter when it starts and cancels itself when the counter it reads is no longer that value, so
//! a cancel can never be lost by arriving between two checks, and a cancel clicked while the agent
//! was idle cannot kill the turn that starts afterwards.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

use crate::cancel::{Cancel, CancelWatch};
use crate::dispatch::ToolReply;
use crate::event::AgentEvent;
use crate::provider::{Message, SecretString};
use crate::rounds;
use crate::tools::Tools;

/// The most chat requests one turn may make.
///
/// A round is one request and the tool results it asked for. Twelve is generous for a coding turn
/// that is converging and still bounded: the loop exists so a model can act on what it read, and a
/// model that has asked for tools twelve times without answering is not going to be helped by a
/// thirteenth request. The turn ends and says the cap was hit, rather than spending without end.
pub const MAX_ROUNDS: usize = 12;

/// Everything an agent needs to talk to one provider.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// The API root; `/chat/completions` is appended by the provider client.
    pub base_url: String,
    pub api_key: SecretString,
    pub model: String,
    /// Prepended to every conversation as the system message.
    pub system: String,
    /// The tools the model may call, read at the start of every request rather than at spawn: a
    /// plugin that arrives mid-session is offered on the next request, and a change never edits
    /// a request already streaming.
    pub tools: Tools,
}

/// The agent loop.
pub struct Agent;

impl Agent {
    /// Start the turn loop and hand back the handle plus the stream of its events.
    ///
    /// The loop finishes when every handle is dropped, which closes the event channel and tells
    /// the consumer that no further turn can arrive.
    pub fn spawn(config: AgentConfig) -> (AgentHandle, Receiver<AgentEvent>) {
        let (prompts, incoming) = mpsc::channel();
        let (events, outgoing) = mpsc::channel();
        let (replies, results) = mpsc::channel();
        let cancel = Arc::new(Cancel::default());
        let counted = Arc::clone(&cancel);
        std::thread::Builder::new()
            .name("perch-agent".to_string())
            .spawn(move || run(&config, &incoming, &events, &results, &counted))
            .expect("spawn the agent turn thread");
        (
            AgentHandle {
                prompts,
                replies,
                cancel,
            },
            outgoing,
        )
    }
}

/// The caller's side of an agent.
///
/// Cloned into whatever runs the tools: a tool result goes back through the same handle the prompt
/// went in, so there is one way to talk to a turn and one place its cancellation lives.
#[derive(Clone)]
pub struct AgentHandle {
    prompts: Sender<String>,
    replies: Sender<ToolReply>,
    cancel: Arc<Cancel>,
}

impl AgentHandle {
    /// Ask for one turn with `text` as the user message.
    ///
    /// A handle whose loop has stopped drops the prompt; there is nobody left to run it, and the
    /// event channel has already closed, which is what tells the caller.
    pub fn prompt(&self, text: String) {
        let _ = self.prompts.send(text);
    }

    /// Answer one call the turn asked for, and report whether the turn was still there.
    ///
    /// Exactly the calls emitted are expected, in any order; a reply for a call the turn is not
    /// waiting for is dropped, so answering late costs nothing but a wasted message.
    pub fn reply(&self, reply: ToolReply) -> bool {
        self.replies.send(reply).is_ok()
    }

    /// A view of this agent's cancellation, for the thread that runs its tools.
    pub fn watch(&self) -> CancelWatch {
        CancelWatch::new(Arc::clone(&self.cancel))
    }

    /// Stop the running turn.
    ///
    /// Cooperative and lossless: the counter only ever increases, so a turn that checks it before
    /// the request, between every chunk, while waiting for a tool result, and once more at the end
    /// cannot miss a cancel that happened after it started. The turn still ends with one
    /// `TurnEnd`, as `Cancelled`.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }
}

/// The loop itself: one turn per prompt, each ending with exactly one `TurnEnd`.
fn run(
    config: &AgentConfig,
    prompts: &Receiver<String>,
    events: &Sender<AgentEvent>,
    results: &Receiver<ToolReply>,
    cancel: &Cancel,
) {
    let mut history = vec![Message::system(config.system.clone())];
    while let Ok(prompt) = prompts.recv() {
        // A reply left over from a turn that was cancelled while its tool was still running
        // belongs to no question this turn asks.
        while results.try_recv().is_ok() {}
        let generation = cancel.begin();
        let (reason, usage) = rounds::one(
            config,
            &mut history,
            &prompt,
            events,
            results,
            cancel,
            generation,
        );
        // The only place a turn ends, for every reason, which is what makes "exactly one" true.
        if events.send(AgentEvent::TurnEnd { reason, usage }).is_err() {
            return;
        }
    }
}
