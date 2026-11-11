//! The last request of a spent turn: what the model is told, and how the answer is read.

use std::sync::mpsc::Sender;

use crate::Chunk;
use crate::event::{AgentEvent, Usage};
use crate::open::{Opened, open as open_stream};
use crate::provider::{ChatRequest, Message};

/// What the model is told when the steps are spent.
const STEPS_SPENT: &str = "CRITICAL - MAXIMUM STEPS REACHED

The maximum number of steps for this turn has been reached and the tools are disabled. Reply with
text only: say what was accomplished, what is still unfinished, and what should happen next. Any
tool call is a violation of this instruction.";

/// The last request of a spent turn: text only, so the turn ends with words rather than an error.
pub(crate) fn report(
    config: &crate::AgentConfig,
    history: &mut Vec<Message>,
    events: &Sender<AgentEvent>,
    stopped: &dyn Fn() -> bool,
    usage: &mut Usage,
) -> std::result::Result<(), Outcome> {
    history.push(Message::user(STEPS_SPENT.to_string()));
    let request = ChatRequest {
        base_url: config.endpoint.base_url(),
        api_key: config.endpoint.api_key(),
        model: config.model.current(),
        messages: history.clone(),
        // No tools: the point of this request is to stop calling them.
        tools: Vec::new(),
    };
    let chunks = match open_stream(&request, stopped) {
        Opened::Stream(chunks) => chunks,
        Opened::Cancelled => return Err(Outcome::Cancelled),
        Opened::Failed(error) => return Err(Outcome::Failed(error.to_string())),
    };
    let mut text = String::new();
    for chunk in chunks {
        if stopped() {
            return Err(Outcome::Cancelled);
        }
        match chunk {
            Ok(Chunk::Text(delta)) => {
                text.push_str(&delta);
                if events.send(AgentEvent::TextDelta(delta)).is_err() {
                    return Err(Outcome::Cancelled);
                }
            }
            Ok(Chunk::Usage { input, output }) => {
                usage.input = usage.input.saturating_add(input);
                usage.output = usage.output.saturating_add(output);
            }
            // A call here is a violation of the instruction above, and running it would undo the
            // only reason this request exists. The fragments are dropped with the reply.
            Ok(Chunk::ToolCall { .. }) => {}
            // A spent turn reports what it reached; its thinking is not part of that report.
            Ok(Chunk::Reasoning(_)) => {}
            Err(error) => return Err(Outcome::Failed(error.to_string())),
        }
    }
    if !text.is_empty() {
        history.push(Message::assistant(text));
    }
    Ok(())
}

/// How a spent turn ended.
pub(crate) enum Outcome {
    Cancelled,
    Failed(String),
}
