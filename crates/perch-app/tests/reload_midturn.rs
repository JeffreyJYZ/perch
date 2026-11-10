//! A reload that lands while a turn is streaming.
//!
//! The turn is a conversation with the model, and the tools it did not call are the only thing the
//! reload changes. So the turn must still end exactly once, with the reply the model sent and the
//! request count it had — the swap lands on the next call, not on this turn's transcript, and the
//! request already out carries the tool list it was built with.

#[path = "../../perch-agent/tests/support/mod.rs"]
mod support;

use std::path::Path;
use std::sync::atomic::Ordering;
use std::time::Duration;

use perch_app::{AppState, ProviderDefaults};
use perch_store::SessionEvent;
use perch_ui::Item;

/// How long the turn is given to finish. Bounded, not timed.
const TIMEOUT: Duration = Duration::from_secs(10);

/// The plugin installed while the reply is streaming.
fn plugin_wasm() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../perch-plugins/fixtures/counter.wasm")
}

#[test]
fn a_reload_mid_turn_leaves_the_turn_alone() {
    let server = support::serve("tests/fixtures/live_slow.txt");
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
    // Wait for the reply to be streaming: the request is out and being answered, which is the only
    // moment at which a reload can be said to have arrived mid-request rather than before one.
    let mut streaming = false;
    for _ in 0..100 {
        if app.pump(Duration::from_millis(50)).changed {
            streaming = true;
            break;
        }
    }
    assert!(streaming, "the reply never started streaming");

    // The tools change now, so the reload is real: the live list gains the plugin's tool.
    let plugins = data.path().join(perch_app::settings::PLUGINS);
    std::fs::create_dir_all(&plugins).unwrap();
    std::fs::copy(plugin_wasm(), plugins.join("counter.wasm")).unwrap();
    assert!(app.reload(), "the reload could not read the settings file");
    assert!(
        app.tools().iter().any(|spec| spec.name == "echo"),
        "the reload did not change the live tools, so the assertion below would prove nothing"
    );

    assert!(
        app.pump_until(TIMEOUT, |session| session.turns_ended() == 1),
        "the turn did not finish within {TIMEOUT:?}"
    );
    // The request that was already out is read back as it was sent: the tool that arrived during
    // the stream is not in it, whatever the live list says now.
    let sent = server
        .bodies
        .lock()
        .unwrap()
        .first()
        .cloned()
        .unwrap_or_default();
    assert!(
        !sent.contains("\"name\":\"echo\""),
        "the mid-stream reload edited the request already in flight: {sent}"
    );
    assert!(
        sent.contains("\"name\":\"bash\""),
        "the streaming request did not carry the built-ins: {sent}"
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
            matches!(item, Item::Assistant { text } if text.contains("Warming up the reply"))
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
