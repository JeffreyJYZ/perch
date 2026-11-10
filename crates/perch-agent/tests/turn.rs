//! Turn-loop tests against recorded SSE bodies served from `tests/fixtures`.
//!
//! A turn that calls a tool is not over: the loop asks again with the results in the conversation,
//! so every test here that sees a tool call also answers it, exactly as the app's dispatcher does.

mod support;

use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use perch_agent::{
    Agent, AgentConfig, AgentEvent, AgentHandle, EndReason, MAX_ROUNDS, Message, SecretString,
    ToolCall, ToolReply, ToolSpec, Tools, Usage,
};
use serde_json::json;

/// How long the tests wait for an event that should already be on its way.
const EVENT_TIMEOUT: Duration = Duration::from_secs(5);

/// How long `collect` keeps reading after the turn's first ending.
const AFTER_END_TIMEOUT: Duration = Duration::from_millis(200);

/// An agent pointed at `fixture`, offering the tool `sse_simple.txt` calls.
fn agent_against(fixture: &str) -> (AgentHandle, Receiver<AgentEvent>) {
    agent_at(&support::fixture_server(fixture))
}

/// The same agent, against a server the test is already holding.
fn agent_at(base_url: &str) -> (AgentHandle, Receiver<AgentEvent>) {
    Agent::spawn(AgentConfig {
        base_url: base_url.to_string(),
        api_key: SecretString::from("test-key"),
        model: "test/model".to_string(),
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
fn answer(handle: &AgentHandle, calls: &[ToolCall], text: &str) {
    for call in calls {
        let sent = handle.reply(ToolReply {
            message: Message::tool_result(call.id.clone(), text),
            ok: true,
        });
        assert!(sent, "the turn should still be waiting for {}", call.id);
    }
}

/// The events up to and including the turn's first tool call, and that call.
fn until_call(events: &Receiver<AgentEvent>) -> (Vec<AgentEvent>, ToolCall) {
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
fn collect_from(events: &Receiver<AgentEvent>, earlier: Vec<AgentEvent>) -> Seen {
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
fn collect(events: &Receiver<AgentEvent>) -> Seen {
    collect_from(events, Vec::new())
}

/// The events one turn produced.
#[derive(Default)]
struct Seen {
    events: Vec<AgentEvent>,
}

impl Seen {
    fn tool_calls(&self) -> Vec<&ToolCall> {
        self.events
            .iter()
            .filter_map(|event| match event {
                AgentEvent::ToolCall(call) => Some(call),
                _ => None,
            })
            .collect()
    }

    fn deltas(&self) -> Vec<&str> {
        self.events
            .iter()
            .filter_map(|event| match event {
                AgentEvent::TextDelta(text) => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    fn ends(&self) -> Vec<&EndReason> {
        self.events
            .iter()
            .filter_map(|event| match event {
                AgentEvent::TurnEnd { reason, .. } => Some(reason),
                _ => None,
            })
            .collect()
    }

    fn end(&self) -> &EndReason {
        self.ends()
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the turn ended without a TurnEnd: {:?}", self.events))
    }

    fn usage(&self) -> Usage {
        self.events
            .iter()
            .find_map(|event| match event {
                AgentEvent::TurnEnd { usage, .. } => Some(*usage),
                _ => None,
            })
            .unwrap_or_default()
    }
}

#[test]
fn tool_call_fragments_assemble_into_valid_json_or_end_errored() {
    let (handle, events) = agent_against("tests/fixtures/sse_simple.txt");
    handle.prompt("go".into());
    let (earlier, call) = until_call(&events);
    assert_eq!(call.name, "read");
    assert_eq!(call.args["path"], "src/main.rs");
    answer(&handle, std::slice::from_ref(&call), "fn main() {}");
    let seen = collect_from(&events, earlier);

    assert_eq!(seen.end(), &EndReason::Done);
    assert_eq!(seen.ends().len(), 1);
    // Both responses cost something, and a turn is one turn: the usage is their sum.
    assert_eq!(
        seen.usage(),
        Usage {
            input: 42,
            output: 7
        }
    );
}

#[test]
fn a_tool_result_is_sent_back_and_the_turn_asks_again() {
    let fixture = support::serve("tests/fixtures/sse_simple.txt");
    let (handle, events) = agent_at(&fixture.base_url);
    handle.prompt("go".into());
    let (earlier, call) = until_call(&events);
    answer(&handle, std::slice::from_ref(&call), "fn main() {}");
    let seen = collect_from(&events, earlier);

    assert_eq!(seen.end(), &EndReason::Done);
    assert_eq!(
        fixture.requests.load(Ordering::SeqCst),
        2,
        "one request for the call, one for the answer"
    );
    let bodies = fixture.bodies.lock().unwrap().clone();
    let second = bodies
        .get(1)
        .unwrap_or_else(|| panic!("no second request: {bodies:?}"));
    assert!(
        second.contains("\"role\":\"assistant\"") && second.contains("\"tool_calls\""),
        "the second request repeats the call the model made: {second}"
    );
    assert!(
        second.contains("\"role\":\"tool\""),
        "the result is a tool message: {second}"
    );
    assert!(
        second.contains("\"tool_call_id\":\"call_1\""),
        "the tool message carries the call's id: {second}"
    );
    assert!(
        second.contains("fn main() {}"),
        "the tool message carries the result's text: {second}"
    );
    assert_eq!(
        seen.deltas().last().copied(),
        Some("It is a main."),
        "the turn ends on the model's own words"
    );
}

#[test]
fn a_cancel_while_waiting_for_a_tool_result_asks_nothing_further() {
    let fixture = support::serve("tests/fixtures/sse_simple.txt");
    let (handle, events) = agent_at(&fixture.base_url);
    handle.prompt("go".into());
    let (earlier, call) = until_call(&events);

    handle.cancel();
    answer(&handle, std::slice::from_ref(&call), "fn main() {}");
    let seen = collect_from(&events, earlier);

    assert_eq!(seen.ends().len(), 1, "a cancelled turn ends exactly once");
    assert_eq!(seen.end(), &EndReason::Cancelled);
    assert_eq!(
        seen.tool_calls().len(),
        1,
        "no call was made after the cancel"
    );
    assert_eq!(
        fixture.requests.load(Ordering::SeqCst),
        1,
        "a cancelled turn asks the model nothing further"
    );
}

#[test]
fn a_turn_that_keeps_asking_for_tools_stops_at_the_cap() {
    let fixture = support::serve("tests/fixtures/sse_loop.txt");
    let (handle, events) = agent_at(&fixture.base_url);
    handle.prompt("go".into());
    let mut seen = Seen::default();
    loop {
        let Ok(event) = events.recv_timeout(EVENT_TIMEOUT) else {
            panic!("the turn neither answered nor ended: {:?}", seen.events);
        };
        if let AgentEvent::ToolCall(call) = &event {
            answer(&handle, std::slice::from_ref(call), "still going");
        }
        let ended = matches!(event, AgentEvent::TurnEnd { .. });
        seen.events.push(event);
        if ended {
            break;
        }
    }
    let EndReason::Errored(why) = seen.end() else {
        panic!("a turn that never answers must say so: {:?}", seen.end());
    };
    assert!(
        why.contains(&MAX_ROUNDS.to_string()),
        "the ending names the cap it hit: {why}"
    );
    assert_eq!(
        fixture.requests.load(Ordering::SeqCst),
        MAX_ROUNDS,
        "the cap is a request count, and no request was made past it"
    );
    assert_eq!(
        seen.tool_calls().len(),
        MAX_ROUNDS,
        "every round asked for one tool"
    );
}

#[test]
fn cancel_yields_exactly_one_cancelled_end() {
    let fixture = support::serve("tests/fixtures/sse_slow.txt");
    let (handle, events) = agent_at(&fixture.base_url);
    handle.prompt("go".into());
    // Wait for the reply to start before cancelling. A cancel that landed before the request
    // went out would pass this test without ever testing a stop mid-stream, which is the case
    // that actually has to work.
    let first = events
        .recv_timeout(EVENT_TIMEOUT)
        .expect("the reply should start streaming");
    assert!(
        matches!(&first, AgentEvent::TextDelta(_)),
        "expected text before the cancel, got {first:?}"
    );
    handle.cancel();
    let seen = collect_from(&events, vec![first]);
    assert_eq!(seen.ends().len(), 1);
    assert_eq!(seen.end(), &EndReason::Cancelled);
    // The fixture is paced, so a turn that ignored the cancel would consume every event before
    // ending. Seeing the server still holding events back is what proves the stop was mid-stream.
    let written = fixture.written.load(Ordering::SeqCst);
    assert!(
        written < fixture.total,
        "the cancel landed only after all {written} events were served"
    );
}

#[test]
fn a_cut_stream_ends_errored_with_the_reason() {
    let (handle, events) = agent_against("tests/fixtures/sse_cut.txt");
    handle.prompt("go".into());
    let seen = collect(&events);
    assert_eq!(seen.ends().len(), 1);
    match seen.end() {
        EndReason::Errored(reason) => {
            assert!(!reason.is_empty(), "an errored turn must say why it failed");
        }
        other => panic!("expected the cut stream to end errored, got {other:?}"),
    }
}
