//! The rounds of a turn: a call assembled from fragments, its result sent back, and the cap.

use std::sync::atomic::Ordering;

use perch_agent::{EndReason, MAX_ROUNDS, Usage};

use crate::harness::{
    EVENT_TIMEOUT, Seen, agent_against, agent_at, answer, collect_from, until_call,
};

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
    let fixture = crate::support::serve("tests/fixtures/sse_simple.txt");
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
fn a_turn_that_keeps_asking_for_tools_stops_at_the_cap() {
    let fixture = crate::support::serve("tests/fixtures/sse_loop.txt");
    let (handle, events) = agent_at(&fixture.base_url);
    handle.prompt("go".into());
    let mut seen = Seen::default();
    loop {
        let Ok(event) = events.recv_timeout(EVENT_TIMEOUT) else {
            panic!("the turn neither answered nor ended: {:?}", seen.events);
        };
        if let perch_agent::AgentEvent::ToolCall(call) = &event {
            answer(&handle, std::slice::from_ref(call), "still going");
        }
        let ended = matches!(event, perch_agent::AgentEvent::TurnEnd { .. });
        seen.events.push(event);
        if ended {
            break;
        }
    }
    // Steps spent is not an error: the turn asks once more with the tools taken away, so the model
    // can say what it reached. A turn that stalls silently is worse than one that reports.
    assert_eq!(
        seen.end(),
        &EndReason::Done,
        "a spent turn ends with the model's report, not an error"
    );
    assert_eq!(
        fixture.requests.load(Ordering::SeqCst),
        MAX_ROUNDS + 1,
        "the cap is one request per round plus the closing report"
    );
    assert_eq!(
        seen.tool_calls().len(),
        MAX_ROUNDS,
        "every round asked for one tool, and the closing request asked for none"
    );
    // The closing request is what makes the ending a report: no tools, and the instruction that
    // says why the tools are gone.
    let bodies = fixture.bodies.lock().unwrap().clone();
    let closing = bodies.last().expect("the closing request arrived");
    assert!(
        !closing.contains("\"tools\""),
        "the closing request still offers the tools it was meant to take away: {closing}"
    );
    assert!(
        closing.contains("MAXIMUM STEPS REACHED"),
        "the model was not told why the tools went: {closing}"
    );
}
