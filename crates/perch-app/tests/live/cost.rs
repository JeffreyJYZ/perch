//! What the last turn cost is a fact about that turn.
//!
//! The fixture reports the same usage for every request, so the only thing that differs between
//! the two turns below is the model each one named — which is exactly what the price must follow.

use std::sync::Arc;
use std::time::{Duration, Instant};

use perch_agent::{ModelTransport, Result as AgentResult};
use perch_app::AppState;

use crate::harness::{TIMEOUT, app_against};

/// Two models that charge ten times apart, and one that states no price at all.
const LISTED: &str = r#"{"data":[
    {"id":"test/model","context_length":100000,
     "pricing":{"prompt":"0.00001","completion":"0.00002"}},
    {"id":"other/model","context_length":100000,
     "pricing":{"prompt":"0.0001","completion":"0.0002"}},
    {"id":"plain/model","context_length":100000}
]}"#;

/// A transport that answers [`LISTED`], so the listing opens no socket.
struct Listed;

impl ModelTransport for Listed {
    fn get(&self, _url: &str, _api_key: &str) -> AgentResult<Vec<u8>> {
        Ok(LISTED.as_bytes().to_vec())
    }
}

/// An app over the repeatable fixture, with the listing's windows and prices remembered.
///
/// The server and both directories are returned because the app's session log and the tool's root
/// are real paths until the test ends.
fn priced_app() -> (
    AppState,
    crate::support::Fixture,
    tempfile::TempDir,
    tempfile::TempDir,
) {
    let (app, server, data, worktree) = app_against("tests/fixtures/live_repeat.txt");
    let mut app = app.with_listings(Arc::new(Listed));
    app.start_models();
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        match app.models_ready() {
            Some(Ok(_)) => return (app, server, data, worktree),
            Some(Err(words)) => panic!("the listing failed: {words}"),
            None => std::thread::sleep(Duration::from_millis(2)),
        }
    }
    panic!("the listing never answered");
}

#[test]
fn a_turn_keeps_the_price_of_the_model_it_asked_for() {
    let (mut app, _server, _data, _worktree) = priced_app();

    app.prompt("first").unwrap();
    assert!(
        app.pump_until(TIMEOUT, |session| session.turns_ended() == 1),
        "the first turn should end within {TIMEOUT:?}"
    );

    // The fixture reports 9 in and 2 out: 9 × $0.00001 + 2 × $0.00002 = $0.00013.
    let first = app.turn_cost().expect("the first turn is priced");
    assert!(
        (first - 1.3e-4).abs() < 1e-12,
        "the first turn costs $0.00013 at test/model's rates, got {first:?}"
    );

    app.choose_model("other/model").unwrap();
    assert_eq!(
        app.turn_cost(),
        Some(first),
        "picking another model does not reprice the turn that already ran"
    );

    app.prompt("second").unwrap();
    assert!(
        app.pump_until(TIMEOUT, |session| session.turns_ended() == 2),
        "the second turn should end within {TIMEOUT:?}"
    );

    // The same 9 in and 2 out, at other/model's ten-times-higher rates.
    let second = app.turn_cost().expect("the second turn is priced");
    assert!(
        (second - 1.3e-3).abs() < 1e-12,
        "the second turn costs $0.0013 at other/model's rates, got {second:?}"
    );
    assert!(
        second > first,
        "the turn that just ran is charged at the model it asked for: {second:?} vs {first:?}"
    );
}

#[test]
fn the_meter_keeps_showing_the_price_of_the_turn_that_ran() {
    let (mut app, _server, _data, _worktree) = priced_app();

    app.prompt("first").unwrap();
    assert!(
        app.pump_until(TIMEOUT, |session| session.turns_ended() == 1),
        "the first turn should end within {TIMEOUT:?}"
    );
    app.choose_model("other/model").unwrap();

    // The meter draws its three numbers from the app: the same turn, the same price it was
    // charged, even though the composer now asks for a model that charges ten times more.
    let shell = perch_ui::Shell::new(app.metas(), 0).with_live(app);
    let meter =
        perch_ui::shell::meter_words(shell.tokens_used(), shell.model_limit(), shell.turn_cost());
    assert_eq!(
        meter.as_deref(),
        Some("11 (0%) · $0.0001"),
        "the meter keeps the finished turn's own price, not the model now chosen"
    );
}

#[test]
fn a_turn_at_a_model_with_no_price_keeps_no_cost() {
    let (mut app, _server, _data, _worktree) = priced_app();
    app.choose_model("plain/model").unwrap();

    app.prompt("hello").unwrap();
    assert!(
        app.pump_until(TIMEOUT, |session| session.turns_ended() == 1),
        "the turn should end within {TIMEOUT:?}"
    );

    assert_eq!(
        app.turn_cost(),
        None,
        "a turn whose model stated no price has no bill, however many tokens it used"
    );

    app.choose_model("other/model").unwrap();
    assert_eq!(
        app.turn_cost(),
        None,
        "picking a priced model afterwards does not invent a bill for the turn already run"
    );
}
