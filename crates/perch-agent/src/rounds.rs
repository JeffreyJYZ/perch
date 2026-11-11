//! One turn: request, stream, run the tools it asked for, and ask again.
//!
//! A turn is a short conversation with the model, not one request. The model answers with words,
//! or with calls; a call is only useful if its result goes back into the conversation, so every
//! call emitted is answered and the next request carries the assistant's call and the tool
//! messages beside it. The turn ends when the model answers without calling anything, or at
//! [`MAX_ROUNDS`], because a loop with no ceiling is a way to spend without end.
//!
//! Nothing here emits a `TurnEnd`: the loop owns that, so a new ending cannot be invented while
//! returning from here.

use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, Sender};

use serde_json::Value;

use crate::Chunk;
use crate::cancel::Cancel;
use crate::dispatch::ToolReply;
use crate::event::{AgentEvent, EndReason, ToolCall, Usage};
use crate::open::{Opened, open as open_stream};
use crate::provider::{ChatRequest, Message, ToolCallRecord};
use crate::replies::{self, Lost};
use crate::report::{Outcome, report as closing_report};
use crate::turn::MAX_ROUNDS;

/// Run one turn and report how it ended and what it cost.
///
/// `generation` is the turn's cancellation generation: once the counter no longer matches it, the
/// turn stops at the next check and leaves the conversation as it was.
#[allow(
    clippy::too_many_arguments,
    reason = "the agent's own state, read once and never rebuilt"
)]
pub(crate) fn one(
    config: &crate::AgentConfig,
    history: &mut Vec<Message>,
    prompt: &str,
    events: &Sender<AgentEvent>,
    results: &Receiver<ToolReply>,
    cancel: &Cancel,
    generation: u64,
) -> (EndReason, Usage) {
    let stopped = || cancel.stopped(generation);
    let mut usage = Usage::default();
    history.push(Message::user(prompt.to_string()));
    for _ in 0..MAX_ROUNDS {
        if stopped() {
            return (EndReason::Cancelled, usage);
        }
        let request = ChatRequest {
            base_url: config.endpoint.base_url(),
            api_key: config.endpoint.api_key(),
            model: config.model.current(),
            messages: history.clone(),
            // The snapshot for this request, taken as it starts. A reload that lands while the
            // reply streams changes the next request, never this one.
            tools: config.tools.specs(),
        };
        let chunks = match open_stream(&request, &stopped) {
            Opened::Stream(chunks) => chunks,
            Opened::Cancelled => return (EndReason::Cancelled, usage),
            Opened::Failed(why) => return (EndReason::Errored(why), usage),
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
                // A turn costs what all of its rounds cost; the caller asked for one turn.
                Ok(Chunk::Usage { input, output }) => {
                    usage.input = usage.input.saturating_add(input);
                    usage.output = usage.output.saturating_add(output);
                }
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
        let content = (!text.is_empty()).then_some(text);
        if calls.is_empty() {
            // The model answered in words, which is the end of the turn.
            if let Some(content) = content {
                history.push(Message::assistant(content));
            }
            return (EndReason::Done, usage);
        }
        // The call the model made goes back into the conversation before its result, or the next
        // request would answer a call the model cannot see it made.
        history.push(Message::assistant_with_tools(content, records));
        for call in &calls {
            if events.send(AgentEvent::ToolCall(call.clone())).is_err() {
                return (EndReason::Cancelled, usage);
            }
        }
        match replies::collect(results, &calls, &stopped) {
            Ok(messages) => history.extend(messages),
            Err(Lost::Cancelled) => return (EndReason::Cancelled, usage),
            Err(Lost::Gone) => {
                return (
                    EndReason::Errored(
                        "the tool runner stopped before it answered every call".to_string(),
                    ),
                    usage,
                );
            }
        }
    }
    // Steps are spent. Ask once more, with the tools taken away, so the turn ends with a report of
    // what it reached instead of an error — the same last resort every harness uses, and the
    // difference between a turn that stalls and one that says where it got to.
    match closing_report(config, history, events, &stopped, &mut usage) {
        Ok(()) => (EndReason::Done, usage),
        Err(Outcome::Cancelled) => (EndReason::Cancelled, usage),
        Err(Outcome::Failed(why)) => (EndReason::Errored(why), usage),
    }
}
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
