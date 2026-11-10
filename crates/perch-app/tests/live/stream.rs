//! A submitted prompt streams into the column and lands in the log.

use perch_store::SessionEvent;
use perch_ui::Item;

use crate::harness::{TIMEOUT, app_against};

#[test]
fn submitting_a_prompt_streams_the_reply_and_writes_the_log() {
    let (mut app, _server, _data, _worktree) = app_against("tests/fixtures/live_text.txt");

    app.prompt("what is this?").unwrap();
    assert!(
        app.pump_until(TIMEOUT, |session| session.turns_ended() == 1),
        "the turn should end within {TIMEOUT:?}"
    );

    let items = app.active_session().unwrap().transcript().items();
    assert!(
        items
            .iter()
            .any(|item| matches!(item, Item::Assistant { text } if text.contains("Hello from the fixture"))),
        "the reply streams into one assistant item: {items:?}"
    );
    assert!(
        items
            .iter()
            .any(|item| matches!(item, Item::User { text } if text == "what is this?")),
        "the question is the turn's first item: {items:?}"
    );

    let events = app.active_session().unwrap().log.events().unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, SessionEvent::User { text } if text == "what is this?")),
        "the question is in the log: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, SessionEvent::Assistant { text } if text.contains("Hello from the fixture"))),
        "the reply is in the log: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, SessionEvent::TurnEnd { reason, .. } if reason == "done")),
        "the ending is in the log: {events:?}"
    );
}
