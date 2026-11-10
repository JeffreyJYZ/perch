//! A reload that lands while a turn is streaming.
//!
//! The turn is a conversation with the model, and the tools it did not call are the only thing the
//! reload changes. So the turn must still end exactly once, with the reply the model sent and the
//! request count it had — the swap lands on the next call, not on this turn's transcript.

#[path = "../../perch-agent/tests/support/mod.rs"]
mod support;

use std::sync::atomic::Ordering;
use std::time::Duration;

use perch_app::{AppState, ProviderDefaults};
use perch_store::SessionEvent;
use perch_ui::Item;

/// How long the turn is given to finish. Bounded, not timed.
const TIMEOUT: Duration = Duration::from_secs(10);

#[test]
fn a_reload_mid_turn_leaves_the_turn_alone() {
    let server = support::serve("tests/fixtures/live_text.txt");
    let data = tempfile::tempdir().unwrap();
    let worktree = tempfile::tempdir().unwrap();
    let mut app = AppState::open_with(
        data.path(),
        ProviderDefaults {
            base_url: server.base_url.clone(),
            api_key: "test-key".to_string(),
            model: "test/model".to_string(),
        },
    )
    .unwrap();
    let index = app.new_session(worktree.path()).unwrap();
    app.switch(index).unwrap();

    app.prompt("what is this?").unwrap();
    // The turn is streaming. A filesystem change now rebuilds the tool list and swaps it in.
    assert!(app.reload(), "the reload could not read the settings file");

    assert!(
        app.pump_until(TIMEOUT, |session| session.turns_ended() == 1),
        "the turn did not finish within {TIMEOUT:?}"
    );
    let session = app.active_session().unwrap();
    let events = session.log.events().unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, SessionEvent::TurnEnd { .. }))
            .count(),
        1,
        "a mid-turn reload ended the turn again: {events:?}"
    );
    let replies = session
        .transcript()
        .items()
        .iter()
        .filter(|item| {
            matches!(item, Item::Assistant { text } if text.contains("Hello from the fixture"))
        })
        .count();
    assert_eq!(replies, 1, "the reply was rewritten or dropped");
    assert_eq!(
        server.requests.load(Ordering::SeqCst),
        1,
        "the reload asked the provider again"
    );
    // The rebuilt list is what the next call will use, and it still has the built-ins.
    assert!(
        app.tools().iter().any(|spec| spec.name == "bash"),
        "the reload lost the built-ins"
    );
}
