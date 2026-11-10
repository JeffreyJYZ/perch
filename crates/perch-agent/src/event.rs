//! What a running turn reports, and the definite ways it can end.

use serde_json::Value;

/// One event from a running turn.
///
/// Everything a consumer needs to draw the turn arrives in order, and `TurnEnd` is always last:
/// a turn that ends without one would leave the UI streaming forever, so the loop emits it from
/// exactly one place.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentEvent {
    /// Assistant text, appended to the open message as it arrives.
    TextDelta(String),
    /// A tool call the model finished asking for, with its arguments already parsed.
    ToolCall(ToolCall),
    /// The outcome of executing one call. Emitted by whatever runs the tools, not by the stream.
    ToolResult { id: String, ok: bool, text: String },
    /// The turn is over. Emitted once per turn, after every other event of that turn.
    TurnEnd { reason: EndReason, usage: Usage },
}

/// One assembled tool call.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    /// The provider's id, or one derived from the call's index when it sent none, so a result
    /// always has something to answer.
    pub id: String,
    pub name: String,
    /// The arguments as JSON. The fragments are joined and parsed once, at the end of the
    /// stream, so a half-received call never reaches the caller.
    pub args: Value,
}

/// Why a turn ended. Every turn ends in exactly one of these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndReason {
    /// The reply was received in full.
    Done,
    /// The caller asked the turn to stop.
    Cancelled,
    /// The turn could not finish; the reason says what went wrong.
    Errored(String),
}

/// Token counts the provider reported for one turn. Zero when it reported none.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input: u32,
    pub output: u32,
}
