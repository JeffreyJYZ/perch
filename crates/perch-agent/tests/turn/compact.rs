//! What a turn does when the provider refuses the conversation as too long.
//!
//! The refusal is its own decision: the same request asked again spends the same money for the
//! same answer, so the conversation is rebuilt smaller and the turn carries on.

use std::sync::atomic::Ordering;

use perch_agent::Error;
use perch_agent::compact::{is_context_length, shorten};
use perch_agent::{AgentEvent, EndReason, Message};

use crate::harness::{agent_at, collect};

/// The bodies a provider actually sends when it will not accept a conversation at this size.
const WORDINGS: [&str; 5] = [
    r#"{"error":{"message":"This model's maximum context length is 128000 tokens"}}"#,
    r#"{"error":{"message":"too many tokens in this request"}}"#,
    r#"{"error":{"message":"maximum context window exceeded"}}"#,
    r#"{"error":{"message":"prompt is too long: 130000 tokens"}}"#,
    r#"{"error":{"code":"context_length_exceeded"}}"#,
];

fn rejected(status: u16, body: &str) -> Error {
    Error::Status {
        status,
        body: body.to_string(),
    }
}

#[test]
fn every_wording_of_a_too_long_request_is_recognised() {
    for body in WORDINGS {
        assert!(
            is_context_length(&rejected(400, body)),
            "a 400 saying this is a length refusal: {body}"
        );
    }
}

#[test]
fn a_plain_bad_request_is_not_a_length_refusal() {
    let bad_request = r#"{"error":{"message":"unknown parameter: temperature"}}"#;
    assert!(
        !is_context_length(&rejected(400, bad_request)),
        "a bad request is a different decision from a long one"
    );
    // The wording alone is not enough: only a 400 is this refusal, not any status that says it.
    assert!(!is_context_length(&rejected(413, WORDINGS[0])));
    assert!(!is_context_length(&Error::Truncated));
}

#[test]
fn shortening_keeps_the_system_message_and_the_exchange_being_answered() {
    let history = vec![
        Message::system("you are a test"),
        Message::user("first"),
        Message::assistant("hello there"),
        Message::user("second"),
    ];
    let smaller = shorten(&history, 20).expect("dropping the first exchange reaches 20 bytes");

    assert_eq!(smaller.exchanges, 1, "one whole exchange went");
    assert_eq!(smaller.messages.len(), 2, "its two messages went with it");
    assert_eq!(
        smaller.messages[0].content.as_deref(),
        Some("you are a test")
    );
    assert_eq!(
        smaller.messages[1].content.as_deref(),
        Some("second"),
        "the question the turn exists to answer is still there"
    );
}

#[test]
fn shortening_refuses_when_only_the_answered_exchange_is_left() {
    let history = vec![Message::system("you are a test"), Message::user("second")];
    assert!(
        shorten(&history, 4).is_none(),
        "there is nothing to drop, so no request is worth sending"
    );
}

#[test]
fn a_too_long_request_is_asked_again_smaller_and_the_turn_finishes() {
    let fixture = crate::support::serve("tests/fixtures/sse_too_long.txt");
    let (handle, events) = agent_at(&fixture.base_url);

    handle.prompt("first".into());
    let first = collect(&events);
    assert_eq!(first.end(), &EndReason::Done, "the first turn is ordinary");

    handle.prompt("second".into());
    let seen = collect(&events);

    assert_eq!(seen.end(), &EndReason::Done);
    assert_eq!(
        seen.deltas().last().copied(),
        Some("It fits now."),
        "the turn ends on the model's answer to the smaller request"
    );
    let dropped: Vec<usize> = seen
        .events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::Compacted { dropped, .. } => Some(*dropped),
            _ => None,
        })
        .collect();
    assert_eq!(dropped, vec![2], "one exchange was dropped, and said so");

    let notice = seen
        .events
        .iter()
        .find_map(AgentEvent::notice)
        .unwrap_or_else(|| panic!("the transcript is told what happened: {:?}", seen.events));
    assert!(
        notice.contains("too long") && notice.contains("dropped"),
        "the words say the conversation was shortened: {notice}"
    );

    let bodies = fixture.bodies.lock().unwrap().clone();
    assert_eq!(bodies.len(), 3, "answered, refused, answered smaller");
    let (refused, smaller) = (&bodies[1], &bodies[2]);
    assert!(
        refused.contains("hello there"),
        "the refusal was the long one"
    );
    assert!(
        smaller.len() < refused.len(),
        "the second request is smaller: {} vs {}",
        smaller.len(),
        refused.len()
    );
    assert!(
        smaller.contains("you are a test") && smaller.contains("second"),
        "the system message and the question survive: {smaller}"
    );
    assert!(
        !smaller.contains("hello there"),
        "the dropped exchange is not sent again: {smaller}"
    );
}

#[test]
fn a_conversation_that_never_fits_stops_with_a_clear_error() {
    let fixture = crate::support::serve("tests/fixtures/sse_too_long_forever.txt");
    let (handle, events) = agent_at(&fixture.base_url);
    for prompt in ["one", "two", "three"] {
        handle.prompt(prompt.into());
        assert_eq!(collect(&events).end(), &EndReason::Done);
    }

    let before = fixture.requests.load(Ordering::SeqCst);
    handle.prompt("four".into());
    let seen = collect(&events);
    let EndReason::Errored(why) = seen.end() else {
        panic!("a turn that cannot fit must say so: {:?}", seen.end());
    };
    assert!(
        why.contains("too long") && why.contains("did not fit"),
        "the ending says what was refused and that shortening did not help: {why}"
    );
    assert_eq!(
        seen.events
            .iter()
            .filter(|event| matches!(event, AgentEvent::Compacted { .. }))
            .count(),
        3,
        "85%, 70% and 50% were each tried with a smaller request"
    );
    // The counter runs for the whole fixture, and the three turns above already spent one request
    // each, so this is the fourth turn's own spend: one ask plus its three smaller ones.
    assert_eq!(
        fixture.requests.load(Ordering::SeqCst) - before,
        4,
        "one ask plus three smaller ones, and no fourth at a size nothing shrank"
    );
}
