//! A turn's agent, and the recorded events it produced.

use std::sync::mpsc::Receiver;
use std::time::Duration;

use perch_agent::{
    Agent, AgentConfig, AgentEvent, AgentHandle, EndReason, Endpoint, Message, Model, ToolCall,
    ToolReply, ToolSpec, Tools, Usage,
};
use serde_json::json;

/// How long the tests wait for an event that should already be on its way.
pub const EVENT_TIMEOUT: Duration = Duration::from_secs(5);

/// How long `collect` keeps reading after the turn's first ending.
const AFTER_END_TIMEOUT: Duration = Duration::from_millis(200);

/// An agent pointed at `fixture`, offering the tool `sse_simple.txt` calls.
pub fn agent_against(fixture: &str) -> (AgentHandle, Receiver<AgentEvent>) {
    agent_at(&crate::support::fixture_server(fixture))
}

/// The same agent, against a server the test is already holding.
pub fn agent_at(base_url: &str) -> (AgentHandle, Receiver<AgentEvent>) {
    agent_with_model(base_url, Model::new("test/model"))
}

/// The same agent, with the model handle kept by the test so it can pick another mid-session.
pub fn agent_with_model(base_url: &str, model: Model) -> (AgentHandle, Receiver<AgentEvent>) {
    Agent::spawn(AgentConfig {
        endpoint: Endpoint::new(base_url.to_string(), "test-key"),
        model,
        system: "you are a test".to_string(),
        tools: Tools::fixed(vec![ToolSpec {
            name: "read".to_string(),
            description: "read a file".to_string(),
            parameters: json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }),
        }]),
    })
}

/// Answers every call the turn asked for, one tool message each, as the dispatcher would.
pub fn answer(handle: &AgentHandle, calls: &[ToolCall], text: &str) {
    for call in calls {
        let sent = handle.reply(ToolReply {
            message: Message::tool_result(call.id.clone(), text),
            ok: true,
        });
        assert!(sent, "the turn should still be waiting for {}", call.id);
    }
}

/// The events up to and including the turn's first tool call, and that call.
pub fn until_call(events: &Receiver<AgentEvent>) -> (Vec<AgentEvent>, ToolCall) {
    let mut kept = Vec::new();
    loop {
        let event = events
            .recv_timeout(EVENT_TIMEOUT)
            .expect("the turn should ask for a tool");
        kept.push(event.clone());
        if let AgentEvent::ToolCall(call) = event {
            return (kept, call);
        }
    }
}

/// Everything up to, and just past, the end of the turn, after the events `earlier` already read.
pub fn collect_from(events: &Receiver<AgentEvent>, earlier: Vec<AgentEvent>) -> Seen {
    let mut seen = Seen { events: earlier };
    while let Ok(event) = events.recv_timeout(EVENT_TIMEOUT) {
        let ended = matches!(event, AgentEvent::TurnEnd { .. });
        seen.events.push(event);
        if ended {
            break;
        }
    }
    // Keep reading past the first ending, so a turn that somehow ended twice shows up as two.
    while let Ok(event) = events.recv_timeout(AFTER_END_TIMEOUT) {
        seen.events.push(event);
    }
    seen
}

/// Everything the turn emits, for a turn whose events have not been read yet.
pub fn collect(events: &Receiver<AgentEvent>) -> Seen {
    collect_from(events, Vec::new())
}

/// The events one turn produced.
#[derive(Default)]
pub struct Seen {
    pub events: Vec<AgentEvent>,
}

impl Seen {
    pub fn tool_calls(&self) -> Vec<&ToolCall> {
        self.events
            .iter()
            .filter_map(|event| match event {
                AgentEvent::ToolCall(call) => Some(call),
                _ => None,
            })
            .collect()
    }

    pub fn deltas(&self) -> Vec<&str> {
        self.events
            .iter()
            .filter_map(|event| match event {
                AgentEvent::TextDelta(text) => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    pub fn ends(&self) -> Vec<&EndReason> {
        self.events
            .iter()
            .filter_map(|event| match event {
                AgentEvent::TurnEnd { reason, .. } => Some(reason),
                _ => None,
            })
            .collect()
    }

    pub fn end(&self) -> &EndReason {
        self.ends()
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the turn ended without a TurnEnd: {:?}", self.events))
    }

    pub fn usage(&self) -> Usage {
        self.events
            .iter()
            .find_map(|event| match event {
                AgentEvent::TurnEnd { usage, .. } => Some(*usage),
                _ => None,
            })
            .unwrap_or_default()
    }
}
