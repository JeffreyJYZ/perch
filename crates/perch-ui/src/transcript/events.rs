//! Folding the agent's events in: deltas into the open reply, calls into cards, and the end of a
//! turn into a definite state.

use perch_agent::AgentEvent;

use super::{Item, ToolStatus, Transcript};

/// Folds one agent event into `model`.
pub(super) fn apply(model: &mut Transcript, event: &AgentEvent) {
    match event {
        AgentEvent::TextDelta(delta) => push_delta(model, delta),
        AgentEvent::ToolCall(call) => {
            // A tool card sits between the text before it and the text after it, so the
            // reply must not keep appending to the paragraph above the card.
            model.open = None;
            model.items.push(Item::Tool {
                id: call.id.clone(),
                name: call.name.clone(),
                status: ToolStatus::Running,
                output: String::new(),
                preview: None,
            });
        }
        AgentEvent::ToolResult { id, ok, text } => {
            if let Some(Item::Tool { status, output, .. }) = model.tool_mut(id) {
                *status = if *ok {
                    ToolStatus::Ok
                } else {
                    ToolStatus::Failed
                };
                *output = text.clone();
            }
        }
        // A compaction is not part of a reply: the model was asked again smaller, so the column
        // says what happened in its own voice and the next text opens a new reply.
        AgentEvent::Compacted { .. } => {
            if let Some(words) = event.notice() {
                model.note(words);
            }
        }
        AgentEvent::TurnEnd { reason, usage } => finish(model, reason, *usage),
    }
}

/// Appends to the open assistant item, or opens a new one.
fn push_delta(model: &mut Transcript, delta: &str) {
    if let Some(index) = model.open
        && let Some(Item::Assistant { text }) = model.items.get_mut(index)
    {
        text.push_str(delta);
        return;
    }
    model.items.push(Item::Assistant {
        text: delta.to_string(),
    });
    model.open = Some(model.items.len() - 1);
}

/// Ends the turn in a definite state: nothing keeps claiming to run, an error is said, and one
/// line reports what the turn did.
fn finish(model: &mut Transcript, reason: &perch_agent::EndReason, usage: perch_agent::Usage) {
    model.open = None;
    let (tools, failed) = count_tools(model);
    match reason {
        perch_agent::EndReason::Done => {}
        perch_agent::EndReason::Cancelled => {
            for item in &mut model.items {
                if let Item::Tool { status, .. } = item
                    && *status == ToolStatus::Running
                {
                    *status = ToolStatus::Cancelled;
                }
            }
            // The reply the turn was writing is the one that was cut short, so it says
            // cancelled where a live one says streaming.
            if let Some(index) = last_assistant(model) {
                model.cancelled.push(index);
            }
            model.note("The turn was cancelled.");
        }
        perch_agent::EndReason::Errored(why) => {
            model.note(format!("The turn failed: {why}"));
        }
    }
    let tokens = usage.input + usage.output;
    let tokens = (tokens > 0).then_some(tokens);
    // A plain answer with neither a call nor a cost prints nothing: there is nothing to report.
    if tools > 0 || tokens.is_some() {
        model.items.push(Item::Summary {
            tools,
            failed,
            tokens,
        });
    }
    model.since = model.items.len();
}

/// How many tool calls this turn ran, and how many of them failed.
fn count_tools(model: &Transcript) -> (usize, usize) {
    let mut tools = 0;
    let mut failed = 0;
    for item in &model.items[model.since.min(model.items.len())..] {
        if let Item::Tool { status, .. } = item {
            tools += 1;
            if *status == ToolStatus::Failed {
                failed += 1;
            }
        }
    }
    (tools, failed)
}

/// The last assistant item of the turn being counted, when it wrote one.
fn last_assistant(model: &Transcript) -> Option<usize> {
    let start = model.since.min(model.items.len());
    model.items[start..]
        .iter()
        .rposition(|item| matches!(item, Item::Assistant { .. }))
        .map(|index| start + index)
}
