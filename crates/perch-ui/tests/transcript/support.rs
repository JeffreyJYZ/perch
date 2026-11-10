//! The events the tests feed a transcript, and the output they feed a card.

use perch_agent::{AgentEvent, EndReason, ToolCall, Usage};

pub fn call(id: &str, name: &str) -> ToolCall {
    ToolCall {
        id: id.to_string(),
        name: name.to_string(),
        args: serde_json::json!({ "path": "src/main.rs" }),
    }
}

/// `lines` numbered lines, each with its newline: longer than a collapsed card shows.
pub fn numbered(lines: usize) -> String {
    (1..=lines).map(|n| format!("line {n}\n")).collect()
}

pub fn end(reason: EndReason) -> AgentEvent {
    AgentEvent::TurnEnd {
        reason,
        usage: Usage::default(),
    }
}
