//! The turn loop.
//!
//! One thread owns the conversation: it takes prompts off a channel, streams a reply from the
//! provider, assembles tool calls, and ends every turn with exactly one [`AgentEvent::TurnEnd`].
//!
//! Cancellation is a monotonic counter rather than a flag. Each turn snapshots the counter when
//! it starts and cancels itself when the counter it reads is no longer that value, so a cancel
//! can never be lost by arriving between two checks, and a cancel clicked while the agent was
//! idle cannot kill the turn that starts afterwards.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};

use serde_json::Value;

use crate::Chunk;
use crate::event::{AgentEvent, EndReason, ToolCall, Usage};
use crate::provider::{ChatRequest, Message, SecretString, ToolCallRecord, ToolSpec};
use crate::stream;

/// Everything an agent needs to talk to one provider.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// The API root; `/chat/completions` is appended by the provider client.
    pub base_url: String,
    pub api_key: SecretString,
    pub model: String,
    /// Prepended to every conversation as the system message.
    pub system: String,
    pub tools: Vec<ToolSpec>,
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
        let cancels = Arc::new(AtomicU64::new(0));
        let counter = Arc::clone(&cancels);
        std::thread::Builder::new()
            .name("perch-agent".to_string())
            .spawn(move || run(&config, &incoming, &events, &counter))
            .expect("spawn the agent turn thread");
        (AgentHandle { prompts, cancels }, outgoing)
    }
}

/// The caller's side of an agent.
#[derive(Clone)]
pub struct AgentHandle {
    prompts: Sender<String>,
    /// Bumped by [`AgentHandle::cancel`]; a turn compares the counter against the value it
    /// captured when it began.
    cancels: Arc<AtomicU64>,
}

impl AgentHandle {
    /// Ask for one turn with `text` as the user message.
    ///
    /// A handle whose loop has stopped drops the prompt; there is nobody left to run it, and the
    /// event channel has already closed, which is what tells the caller.
    pub fn prompt(&self, text: String) {
        let _ = self.prompts.send(text);
    }

    /// Stop the running turn.
    ///
    /// Cooperative and lossless: the counter only ever increases, so a turn that checks it
    /// before the request, between every chunk, and once more at the end cannot miss a cancel
    /// that happened after it started. The turn still ends with one `TurnEnd`, as `Cancelled`.
    pub fn cancel(&self) {
        self.cancels.fetch_add(1, Ordering::SeqCst);
    }
}

/// The loop itself: one turn per prompt, each ending with exactly one `TurnEnd`.
fn run(
    config: &AgentConfig,
    prompts: &Receiver<String>,
    events: &Sender<AgentEvent>,
    cancels: &AtomicU64,
) {
    let mut history = vec![Message::system(config.system.clone())];
    while let Ok(prompt) = prompts.recv() {
        // Captured before the turn does anything, so a cancel from an earlier turn (or one made
        // while the agent was idle) does not apply to this one, and every later cancel does.
        let generation = cancels.load(Ordering::SeqCst);
        let (reason, usage) = turn(config, &mut history, &prompt, events, cancels, generation);
        // The only place a turn ends, for every reason, which is what makes "exactly one" true.
        if events.send(AgentEvent::TurnEnd { reason, usage }).is_err() {
            return;
        }
    }
}

/// Run one turn and report how it ended and what it cost.
///
/// Emits its own text and tool-call events but never a `TurnEnd`; the loop owns that, so a new
/// ending cannot be invented while returning from here.
fn turn(
    config: &AgentConfig,
    history: &mut Vec<Message>,
    prompt: &str,
    events: &Sender<AgentEvent>,
    cancels: &AtomicU64,
    generation: u64,
) -> (EndReason, Usage) {
    let stopped = || cancels.load(Ordering::SeqCst) != generation;
    let mut usage = Usage::default();
    history.push(Message::user(prompt.to_string()));
    if stopped() {
        return (EndReason::Cancelled, usage);
    }
    let request = ChatRequest {
        base_url: config.base_url.clone(),
        api_key: config.api_key.clone(),
        model: config.model.clone(),
        messages: history.clone(),
        tools: config.tools.clone(),
    };
    let chunks = match stream(&request) {
        Ok(chunks) => chunks,
        Err(error) => return (EndReason::Errored(error.to_string()), usage),
    };
    let mut text = String::new();
    let mut fragments: BTreeMap<usize, Fragment> = BTreeMap::new();
    for chunk in chunks {
        if stopped() {
            // Dropping the iterator closes the provider channel, so the request stops too.
            return (EndReason::Cancelled, usage);
        }
        match chunk {
            Ok(Chunk::Text(delta)) => {
                text.push_str(&delta);
                if events.send(AgentEvent::TextDelta(delta)).is_err() {
                    return (EndReason::Cancelled, usage);
                }
            }
            Ok(Chunk::ToolCall {
                index,
                id,
                name,
                args_fragment,
            }) => {
                let call = fragments.entry(index).or_default();
                // Only the first fragment of a call carries the id and name; a later fragment
                // that omits them must not erase what arrived.
                if id.is_some() {
                    call.id = id;
                }
                if name.is_some() {
                    call.name = name;
                }
                call.args.push_str(&args_fragment);
            }
            Ok(Chunk::Usage { input, output }) => usage = Usage { input, output },
            Err(error) => return (EndReason::Errored(error.to_string()), usage),
        }
    }
    // A cancel that arrived with the final chunk still ends the turn as cancelled rather than
    // reporting a reply the caller asked not to see.
    if stopped() {
        return (EndReason::Cancelled, usage);
    }
    let (calls, records) = match assemble(fragments) {
        Ok(calls) => calls,
        Err(reason) => return (EndReason::Errored(reason), usage),
    };
    for call in &calls {
        if events.send(AgentEvent::ToolCall(call.clone())).is_err() {
            return (EndReason::Cancelled, usage);
        }
    }
    if !text.is_empty() || !records.is_empty() {
        let content = if text.is_empty() { None } else { Some(text) };
        history.push(Message::assistant_with_tools(content, records));
    }
    (EndReason::Done, usage)
}

/// The fragments of one tool call, keyed by the provider's call index.
#[derive(Default)]
struct Fragment {
    id: Option<String>,
    name: Option<String>,
    args: String,
}

/// Join every call's fragments and parse its arguments once.
///
/// All calls are parsed before any is returned, so a stream that cut one call short yields an
/// error instead of the complete-looking calls beside it.
fn assemble(
    fragments: BTreeMap<usize, Fragment>,
) -> std::result::Result<(Vec<ToolCall>, Vec<ToolCallRecord>), String> {
    let mut calls = Vec::with_capacity(fragments.len());
    let mut records = Vec::with_capacity(fragments.len());
    for (index, fragment) in fragments {
        let Some(name) = fragment.name else {
            return Err(format!("tool call {index} arrived without a name"));
        };
        // A tool with no parameters legitimately streams no arguments at all.
        let arguments = if fragment.args.trim().is_empty() {
            "{}".to_string()
        } else {
            fragment.args
        };
        let args: Value = serde_json::from_str(&arguments).map_err(|error| {
            format!("tool call {index} ({name}) has invalid arguments: {error}")
        })?;
        let id = fragment.id.unwrap_or_else(|| format!("call_{index}"));
        records.push(ToolCallRecord {
            id: id.clone(),
            name: name.clone(),
            arguments,
        });
        calls.push(ToolCall { id, name, args });
    }
    Ok((calls, records))
}
