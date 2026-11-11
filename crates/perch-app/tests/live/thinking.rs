//! Thinking is shown in its own item, and the answer and the log hold only the answer.

use perch_store::SessionEvent;
use perch_ui::Item;

use crate::harness::{TIMEOUT, app_against, said};

/// The two reasoning deltas the fixture streams, joined as the column shows them.
const THOUGHT: &str = "Let me look at the file before answering. It is probably short.";

#[test]
fn a_thought_is_its_own_item_and_never_enters_the_answer_or_the_log() {
    let (mut app, _server, _data, _worktree) = app_against("tests/fixtures/live_thinking.txt");

    app.prompt("is the file fine?").unwrap();
    assert!(
        app.pump_until(TIMEOUT, |session| session.turns_ended() == 1),
        "the turn should end within {TIMEOUT:?}"
    );

    let items = app.active_session().unwrap().transcript().items();
    let thought = items.iter().find_map(|item| match item {
        Item::Reasoning { text } => Some(text.as_str()),
        _ => None,
    });
    assert_eq!(
        thought,
        Some(THOUGHT),
        "the thought is its own item: {items:?}"
    );

    let answer = said(&app);
    assert_eq!(
        answer, "The file is fine.",
        "the answer is exactly the reply"
    );
    assert!(
        !answer.contains("probably short"),
        "no thought leaks into the answer: {answer}"
    );

    let events = app.active_session().unwrap().log.events().unwrap();
    let logged: String = events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::Assistant { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(logged, "The file is fine.", "the log keeps only the answer");
    assert!(
        !format!("{events:?}").contains("probably short"),
        "no log event carries the thought: {events:?}"
    );
}
