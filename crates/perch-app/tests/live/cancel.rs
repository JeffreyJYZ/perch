//! A cancel mid-stream: one ending, and the column says cancelled.

use perch_store::SessionEvent;
use perch_ui::Item;

use crate::harness::{SETTLE, TIMEOUT, app_against};

#[test]
fn cancelling_ends_the_turn_once_and_the_transcript_says_cancelled() {
    let (mut app, _server, _data, _worktree) = app_against("tests/fixtures/live_slow.txt");

    app.prompt("go").unwrap();
    assert!(
        app.pump_until(TIMEOUT, |session| session
            .transcript()
            .items()
            .iter()
            .any(|item| matches!(item, Item::Assistant { .. }))),
        "the reply should start streaming within {TIMEOUT:?}"
    );

    app.cancel();
    assert!(
        app.pump_until(TIMEOUT, |session| session.turns_ended() >= 1),
        "a cancel still ends the turn"
    );
    // Keep listening long enough for a second ending to arrive and be counted.
    app.pump_until(SETTLE, |_| false);

    let session = app.active_session().unwrap();
    assert_eq!(
        session.turns_ended(),
        1,
        "a cancelled turn ends exactly once"
    );
    let items = session.transcript().items();
    let notes: Vec<&str> = items
        .iter()
        .filter_map(|item| match item {
            Item::Note(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        notes
            .iter()
            .any(|text| text.to_lowercase().contains("cancelled")),
        "the transcript says the turn was cancelled: {items:?}"
    );
    assert_eq!(
        notes
            .iter()
            .filter(|text| text.to_lowercase().contains("cancelled"))
            .count(),
        1,
        "and says it once, not once as a note and again as text: {items:?}"
    );
    let events = session.log.events().unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, SessionEvent::TurnEnd { .. }))
            .count(),
        1,
        "the log records one ending: {events:?}"
    );
}
