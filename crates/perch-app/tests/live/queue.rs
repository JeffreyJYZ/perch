//! A prompt sent while a turn runs waits its turn: the reply in flight is never interleaved, and
//! every held message goes out afterwards, in the order it was written.

use perch_store::SessionEvent;
use perch_ui::Item;

use crate::harness::{TIMEOUT, app_against};

/// Every user row's text, in the order the column holds them.
fn asked(app: &perch_app::AppState) -> Vec<String> {
    app.active_session()
        .unwrap()
        .transcript()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::User { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn three_prompts_sent_mid_turn_all_go_out_after_it_in_order() {
    let (mut app, _server, _data, _worktree) = app_against("tests/fixtures/live_queue.txt");

    app.prompt("first").unwrap();
    // The first turn is still streaming, so these are held rather than sent.
    for text in ["second", "third", "fourth"] {
        app.prompt(text).unwrap();
    }

    assert_eq!(
        asked(&app),
        ["first"],
        "a held prompt is not a row yet: the running turn owns the column"
    );

    assert!(
        app.pump_until(TIMEOUT, |session| session.turns_ended() == 4),
        "all four turns should run within {TIMEOUT:?}"
    );

    assert_eq!(
        asked(&app),
        ["first", "second", "third", "fourth"],
        "every held message went out, in the order it was written"
    );

    // The log is the durable proof of the order: each question lands after the turn before it ended.
    let events = app.active_session().unwrap().log.events().unwrap();
    let order: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::User { text } => Some(format!("user:{text}")),
            SessionEvent::TurnEnd { .. } => Some("end".to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(
        order,
        [
            "user:first",
            "end",
            "user:second",
            "end",
            "user:third",
            "end",
            "user:fourth",
            "end",
        ],
        "no second question reached the log before the first turn ended"
    );
}
