//! What the model is offered, request by request.
//!
//! The tool list comes from a source read at the start of every request, so a tool that arrives
//! mid-session is offered on the next request — and a request already streaming keeps the list it
//! was built with. Both are read off the request bodies the fixture recorded rather than off a
//! struct, which is the only place the answer is real.

mod support;

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use perch_agent::{
    Agent, AgentConfig, AgentEvent, AgentHandle, EndReason, SecretString, ToolSpec, Tools,
};
use serde_json::json;

/// How long a turn may take before the test gives up on it.
const TIMEOUT: Duration = Duration::from_secs(10);

/// The tool every turn in this file starts with.
fn read() -> ToolSpec {
    ToolSpec {
        name: "read".to_string(),
        description: "read a file".to_string(),
        parameters: json!({
            "type": "object",
            "properties": { "path": { "type": "string" } },
            "required": ["path"]
        }),
    }
}

/// The tool that is installed while the agent is running.
fn echo() -> ToolSpec {
    ToolSpec {
        name: "echo".to_string(),
        description: "echo text back".to_string(),
        parameters: json!({
            "type": "object",
            "properties": { "text": { "type": "string" } },
            "required": ["text"]
        }),
    }
}

/// An agent whose offered tools are whatever `shared` holds when a request starts.
fn agent_with(
    base_url: &str,
    shared: &Arc<Mutex<Vec<ToolSpec>>>,
) -> (AgentHandle, Receiver<AgentEvent>) {
    let source = Arc::clone(shared);
    Agent::spawn(AgentConfig {
        base_url: base_url.to_string(),
        api_key: SecretString::from("test-key"),
        model: "test/model".to_string(),
        system: "you are a test".to_string(),
        tools: Tools::live(move || source.lock().unwrap().clone()),
    })
}

/// The body of request number `index`, counted from zero.
fn request(fixture: &support::Fixture, index: usize) -> String {
    let bodies = fixture.bodies.lock().unwrap();
    bodies
        .get(index)
        .unwrap_or_else(|| panic!("the fixture never received request {index}: {bodies:?}"))
        .clone()
}

/// Everything a turn emits, stopping one quiet gap after its ending.
fn turn(events: &Receiver<AgentEvent>) -> Vec<AgentEvent> {
    let mut seen = Vec::new();
    while let Ok(event) = events.recv_timeout(TIMEOUT) {
        let ended = matches!(event, AgentEvent::TurnEnd { .. });
        seen.push(event);
        if ended {
            break;
        }
    }
    while let Ok(event) = events.recv_timeout(Duration::from_millis(100)) {
        seen.push(event);
    }
    seen
}

/// The endings in `seen`, as `Done`/`Cancelled` words for an assertion's message.
fn ending(seen: &[AgentEvent]) -> Option<EndReason> {
    seen.iter().find_map(|event| match event {
        AgentEvent::TurnEnd { reason, .. } => Some(reason.clone()),
        _ => None,
    })
}

#[test]
fn a_tool_added_between_turns_is_offered_in_the_next_request() {
    let fixture = support::serve("tests/fixtures/sse_answers.txt");
    let tools = Arc::new(Mutex::new(vec![read()]));
    let (handle, events) = agent_with(&fixture.base_url, &tools);

    handle.prompt("one".into());
    let first = turn(&events);
    assert_eq!(ending(&first), Some(EndReason::Done));
    let sent = request(&fixture, 0);
    assert!(
        sent.contains("\"name\":\"read\""),
        "the first request does not offer the tool it had: {sent}"
    );
    assert!(
        !sent.contains("\"name\":\"echo\""),
        "a tool that was never installed was offered: {sent}"
    );

    // The plugin arrives while the agent is alive: nothing is respawned, nothing is restarted.
    tools.lock().unwrap().push(echo());

    handle.prompt("two".into());
    let second = turn(&events);
    assert_eq!(ending(&second), Some(EndReason::Done));
    let sent = request(&fixture, 1);
    assert!(
        sent.contains("\"name\":\"echo\""),
        "the second request does not offer the tool added between turns: {sent}"
    );
    assert!(
        sent.contains("\"name\":\"read\""),
        "the second request lost the tool the first one had: {sent}"
    );
}

#[test]
fn a_tool_added_mid_stream_leaves_the_request_in_flight_alone() {
    let fixture = support::serve("tests/fixtures/sse_slow.txt");
    let tools = Arc::new(Mutex::new(vec![read()]));
    let (handle, events) = agent_with(&fixture.base_url, &tools);

    handle.prompt("go".into());
    // Wait for the reply to be streaming: the request is already out and being answered, which is
    // the only moment that makes this a test of the snapshot rather than of ordering.
    let first = events
        .recv_timeout(TIMEOUT)
        .expect("the reply should start streaming");
    assert!(
        matches!(&first, AgentEvent::TextDelta(_)),
        "expected text before the reload, got {first:?}"
    );
    tools.lock().unwrap().push(echo());

    let mut seen = vec![first];
    seen.extend(turn(&events));
    assert_eq!(
        ending(&seen),
        Some(EndReason::Done),
        "the turn did not end once, normally: {seen:?}"
    );
    assert_eq!(
        seen.iter()
            .filter(|event| matches!(event, AgentEvent::TurnEnd { .. }))
            .count(),
        1,
        "the reload ended the turn a second time: {seen:?}"
    );
    let sent = request(&fixture, 0);
    assert!(
        sent.contains("\"name\":\"read\""),
        "the streaming request lost the tool it started with: {sent}"
    );
    assert!(
        !sent.contains("\"name\":\"echo\""),
        "a reload changed a request that was already in flight: {sent}"
    );
    assert_eq!(
        fixture.requests.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the reload asked the provider again"
    );
}
