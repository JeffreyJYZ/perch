//! A session's model: one handle, read at the start of every request.

use std::sync::atomic::Ordering;

use perch_agent::Model;

use crate::harness::{agent_with_model, collect};

/// The `"model"` field of the body the fixture recorded for request `index`.
fn model_of(fixture: &crate::support::Fixture, index: usize) -> String {
    let bodies = fixture.bodies.lock().unwrap();
    let body = bodies
        .get(index)
        .unwrap_or_else(|| panic!("the fixture never received request {index}: {bodies:?}"));
    let parsed: serde_json::Value = serde_json::from_str(body).expect("the body is JSON");
    parsed["model"]
        .as_str()
        .unwrap_or_else(|| panic!("the body names no model: {body}"))
        .to_string()
}

#[test]
fn a_model_picked_between_turns_is_the_next_request_and_not_the_one_before() {
    let fixture = crate::support::serve("tests/fixtures/sse_plain.txt");
    let model = Model::new("first/model");
    let (handle, events) = agent_with_model(&fixture.base_url, model.clone());

    handle.prompt("one".into());
    let first = collect(&events);
    assert_eq!(first.ends().len(), 1, "the first turn ended once");
    assert_eq!(
        fixture.requests.load(Ordering::SeqCst),
        1,
        "the first turn is one request"
    );
    assert_eq!(model_of(&fixture, 0), "first/model");

    // The session's model changes while the agent is idle: the next turn asks for it, and the
    // request already made is untouched.
    model.set("second/model");
    assert_eq!(model.current(), "second/model");

    handle.prompt("two".into());
    let second = collect(&events);
    assert_eq!(second.ends().len(), 1, "the second turn ended once");
    assert_eq!(
        fixture.requests.load(Ordering::SeqCst),
        2,
        "the second turn is one more request"
    );
    assert_eq!(
        model_of(&fixture, 1),
        "second/model",
        "the second request names the model the session picked"
    );
}
