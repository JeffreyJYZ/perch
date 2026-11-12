//! What the model is told when a session is reopened: the earlier exchanges, and nothing at all
//! when there are none.

use std::sync::atomic::Ordering;
use std::time::Duration;

use serde_json::Value;

use crate::harness::{TIMEOUT, exchange, first_request, log, open, place};

/// The messages of the first request the fixture received.
fn sent(fixture: &crate::support::Fixture) -> Vec<Value> {
    let body: Value = serde_json::from_str(&first_request(fixture)).expect("the body is JSON");
    body["messages"]
        .as_array()
        .expect("the request carries messages")
        .clone()
}

#[test]
fn a_reopened_session_sends_the_earlier_exchanges_with_its_first_turn() {
    let (data, worktree) = place();
    log(
        data.path(),
        worktree.path(),
        &[
            exchange("how do I build?", "cargo build."),
            exchange("and test?", "cargo test."),
        ]
        .concat(),
    );
    let (mut app, server) = open(data.path(), "tests/fixtures/live_text.txt");

    app.prompt("continue").unwrap();
    assert!(
        app.pump_until(TIMEOUT, |session| session.turns_ended() == 1),
        "the turn should end within {TIMEOUT:?}"
    );

    let messages = sent(&server);
    assert_eq!(
        messages[0]["role"], "system",
        "the system message comes first"
    );
    let system = messages[0]["content"]
        .as_str()
        .expect("the system message is text");
    let mut after = 0;
    for said in [
        "how do I build?",
        "cargo build.",
        "and test?",
        "cargo test.",
    ] {
        let at = system
            .find(said)
            .unwrap_or_else(|| panic!("{said} is missing from what the model was sent: {system}"));
        assert!(
            at >= after,
            "{said} is out of order in the history: {system}"
        );
        after = at;
    }
    let asked = messages[1..]
        .iter()
        .filter_map(|message| message["content"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        asked,
        vec!["continue"],
        "the history is context, and the question is the only thing asked"
    );
    assert_eq!(
        server.requests.load(Ordering::SeqCst),
        1,
        "one turn is one request: the replay is not re-sent piece by piece"
    );
}

#[test]
fn a_session_with_no_history_sends_the_plain_system_message() {
    let (data, worktree) = place();
    log(data.path(), worktree.path(), &[]);
    let (mut app, server) = open(data.path(), "tests/fixtures/live_text.txt");

    app.prompt("hello").unwrap();
    assert!(
        app.pump_until(TIMEOUT, |session| session.turns_ended() == 1),
        "the turn should end within {TIMEOUT:?}"
    );

    let messages = sent(&server);
    assert_eq!(
        messages.len(),
        2,
        "a system message and the question, and nothing else: {messages:?}"
    );
    let system = messages[0]["content"].as_str().unwrap_or_default();
    assert_eq!(messages[0]["role"], "system");
    assert!(
        !system.contains("Earlier in this session"),
        "an empty log adds no history: {system}"
    );
    assert_eq!(messages[1]["content"], "hello");
}

#[test]
fn opening_a_reopened_session_asks_the_provider_nothing() {
    let (data, worktree) = place();
    log(
        data.path(),
        worktree.path(),
        &[exchange("first", "one"), exchange("second", "two")].concat(),
    );
    let (mut app, server) = open(data.path(), "tests/fixtures/live_text.txt");

    // A frame, so anything a replay did eagerly would have landed by now.
    app.pump(Duration::from_millis(50));
    assert_eq!(
        server.requests.load(Ordering::SeqCst),
        0,
        "a replay is local: opening a session reaches no provider"
    );
    assert!(server.bodies.lock().unwrap().is_empty());
}
