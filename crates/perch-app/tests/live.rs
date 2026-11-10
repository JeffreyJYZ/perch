//! The live session, end to end, against a local server that replays a recorded SSE body.
//!
//! Nothing here reaches a model or the network. Each test proves one promise of the window's
//! session state: a submitted prompt streams into the transcript and lands in the log, a call that
//! needs approval waits for the answer before it runs, its result goes back to the model for a
//! second request, and a cancel ends the turn exactly once.
//!
//! The fixture server is the agent crate's own, shared by path rather than copied, so the app
//! tests and the turn tests speak to the same recorder. It keeps every request body, which is how
//! a test proves what the model was sent rather than what a struct says.

#[path = "../../perch-agent/tests/support/mod.rs"]
mod support;

use std::sync::atomic::Ordering;
use std::time::Duration;

use perch_agent::{Answer, Decision};
use perch_app::{AppState, Pending, ProviderDefaults};
use perch_store::SessionEvent;
use perch_ui::{Item, ToolStatus};
use tempfile::TempDir;

/// How long a test waits for the bridge to deliver something it requires.
const TIMEOUT: Duration = Duration::from_secs(10);
/// How long a test listens for a second `TurnEnd` that must never arrive.
const SETTLE: Duration = Duration::from_millis(300);

/// The provider a fixture server stands in for.
fn defaults(base_url: &str) -> ProviderDefaults {
    ProviderDefaults {
        base_url: base_url.to_string(),
        api_key: "test-key".to_string(),
        model: "test/model".to_string(),
    }
}

/// An app with one session bound to a throwaway worktree, talking to `fixture`.
///
/// Both temp directories outlive the app, because the app's session log and the tool's root are
/// real paths until the test ends. The fixture comes back too, so a test can count its requests
/// and read the messages the model was sent.
fn app_against(fixture: &str) -> (AppState, support::Fixture, TempDir, TempDir) {
    let server = support::serve(fixture);
    let data = tempfile::tempdir().unwrap();
    let worktree = tempfile::tempdir().unwrap();
    let mut app = AppState::open_with(data.path(), defaults(&server.base_url)).unwrap();
    let index = app.new_session(worktree.path()).unwrap();
    app.switch(index).unwrap();
    (app, server, data, worktree)
}

/// The first tool card in the active transcript.
fn tool_card(app: &AppState) -> Option<&Item> {
    app.active_session()?
        .transcript()
        .items()
        .iter()
        .find(|item| matches!(item, Item::Tool { .. }))
}

/// Every assistant item's text, joined, as the column shows it.
fn said(app: &AppState) -> String {
    app.active_session()
        .unwrap()
        .transcript()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::Assistant { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// The body of request number `index`, counted from zero.
fn request(fixture: &support::Fixture, index: usize) -> String {
    let bodies = fixture.bodies.lock().unwrap();
    bodies
        .get(index)
        .unwrap_or_else(|| panic!("the fixture never received request {index}: {bodies:?}"))
        .clone()
}

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

#[test]
fn an_approval_is_raised_and_answering_allow_runs_the_tool() {
    let (mut app, fixture, _data, worktree) = app_against("tests/fixtures/live_write.txt");

    app.prompt("write made.txt").unwrap();
    assert!(
        app.pump_until(TIMEOUT, |session| session.pending().is_some()),
        "the write call should wait for an answer within {TIMEOUT:?}"
    );

    match tool_card(&app).expect("the call is a card in the transcript") {
        Item::Tool {
            id,
            name,
            status,
            output,
            preview,
        } => {
            assert_eq!(id, "call_w1");
            assert_eq!(name, "write");
            assert_eq!(*status, ToolStatus::ApprovalNeeded);
            assert!(
                preview.is_some(),
                "an approval shows the change it approves"
            );
            assert!(output.is_empty(), "nothing has run yet");
        }
        other => panic!("expected a tool card, got {other:?}"),
    }
    assert!(
        !worktree.path().join("made.txt").exists(),
        "the file must not exist before the call is allowed"
    );

    assert!(app.answer(Decision::Allow), "the session was waiting");
    assert!(
        app.pump_until(TIMEOUT, |session| session.turns_ended() == 1),
        "the turn should finish once the call is allowed"
    );

    match tool_card(&app).expect("the card stays in the transcript") {
        Item::Tool { status, output, .. } => {
            assert_eq!(*status, ToolStatus::Ok, "the allowed call ran: {output}");
            assert!(
                output.contains("wrote"),
                "the card shows the result the tool returned: {output}"
            );
        }
        other => panic!("expected a tool card, got {other:?}"),
    }
    assert_eq!(
        std::fs::read_to_string(worktree.path().join("made.txt")).unwrap(),
        "hello from perch"
    );
    let events = app.active_session().unwrap().log.events().unwrap();
    assert!(
        events.iter().any(|event| match event {
            SessionEvent::ToolResult { id, ok, .. } => id == "call_w1" && *ok,
            _ => false,
        }),
        "the result is in the log: {events:?}"
    );

    // The turn is not over when the tool ends: the result goes back to the model, which asks for
    // nothing further and answers.
    assert_eq!(
        fixture.requests.load(Ordering::SeqCst),
        2,
        "the turn asks again with the result in front of it"
    );
    let second = request(&fixture, 1);
    assert!(
        second.contains("\"role\":\"tool\"") && second.contains("\"tool_call_id\":\"call_w1\""),
        "the second request carries a tool message for the call: {second}"
    );
    assert!(
        second.contains("wrote"),
        "the tool message carries what the call returned: {second}"
    );
    assert!(
        said(&app).contains("Wrote it."),
        "the transcript ends on the model's answer: {:?}",
        said(&app)
    );
}

#[test]
fn a_denied_call_reaches_the_model_as_a_tool_message_rather_than_silence() {
    let (mut app, fixture, _data, worktree) = app_against("tests/fixtures/live_write.txt");

    app.prompt("write made.txt").unwrap();
    assert!(
        app.pump_until(TIMEOUT, |session| session.pending().is_some()),
        "the write call should wait for an answer within {TIMEOUT:?}"
    );
    assert!(app.refuse(), "the session was waiting on an approval");
    assert!(
        app.pump_until(TIMEOUT, |session| session.turns_ended() == 1),
        "a refused call still lets the turn finish"
    );

    assert!(
        !worktree.path().join("made.txt").exists(),
        "a denied call must not have written the file"
    );
    assert_eq!(
        fixture.requests.load(Ordering::SeqCst),
        2,
        "a denial is answered, not dropped"
    );
    let second = request(&fixture, 1);
    assert!(
        second.contains("\"role\":\"tool\"") && second.contains("\"tool_call_id\":\"call_w1\""),
        "the denial is a tool message for the call: {second}"
    );
    assert!(
        second.contains("denied"),
        "the model is told the call was denied: {second}"
    );
    match tool_card(&app).expect("the card stays in the transcript") {
        Item::Tool { status, .. } => assert_eq!(*status, ToolStatus::Failed),
        other => panic!("expected a tool card, got {other:?}"),
    }
}

#[test]
fn cancelling_while_a_call_waits_for_approval_runs_nothing_and_ends_once() {
    let (mut app, fixture, _data, worktree) = app_against("tests/fixtures/live_write.txt");

    app.prompt("write made.txt").unwrap();
    assert!(
        app.pump_until(TIMEOUT, |session| session.pending().is_some()),
        "the write call should wait for an answer within {TIMEOUT:?}"
    );
    app.interrupt();
    assert!(
        app.pump_until(TIMEOUT, |session| session.turns_ended() >= 1),
        "a cancel still ends the turn"
    );
    // Keep listening long enough for a second ending, or a second request, to show up.
    app.pump_until(SETTLE, |_| false);

    let session = app.active_session().unwrap();
    assert_eq!(
        session.turns_ended(),
        1,
        "a cancel mid-loop ends the turn exactly once"
    );
    assert!(
        !worktree.path().join("made.txt").exists(),
        "no call runs after the cancel"
    );
    assert_eq!(
        fixture.requests.load(Ordering::SeqCst),
        1,
        "a cancelled turn asks the model nothing further"
    );
    assert_eq!(
        session
            .log
            .events()
            .unwrap()
            .iter()
            .filter(|event| matches!(event, SessionEvent::TurnEnd { .. }))
            .count(),
        1,
        "the log records one ending"
    );
}

#[test]
fn a_question_waits_for_the_answer_and_returns_the_pick() {
    let (mut app, fixture, _data, _worktree) = app_against("tests/fixtures/live_question.txt");

    app.prompt("go on").unwrap();
    assert!(
        app.pump_until(TIMEOUT, |session| session.pending().is_some()),
        "the question should wait for the user within {TIMEOUT:?}"
    );

    let session = app.active_session().unwrap();
    assert!(
        matches!(session.pending(), Some(Pending::Question { options, .. }) if options == &["a".to_string(), "b".to_string()]),
        "the session is waiting on the model's question: {:?}",
        session.pending()
    );
    let shown: Vec<&str> = session
        .transcript()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::Assistant { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        shown.iter().any(|text| text.contains("which one?")),
        "the transcript asks the question: {shown:?}"
    );
    assert!(
        shown.iter().any(|text| text.contains("2. b")),
        "the transcript lists the options: {shown:?}"
    );

    assert!(
        app.answer(Answer::Picks(vec![1])),
        "the session was waiting"
    );
    assert!(
        app.pump_until(TIMEOUT, |session| session.turns_ended() == 1),
        "the turn finishes once the question is answered"
    );
    match tool_card(&app) {
        Some(Item::Tool { output, .. }) => {
            assert_eq!(output, "b", "the model is given the option the user picked")
        }
        other => panic!("expected the question's card, got {other:?}"),
    }
    assert_eq!(
        fixture.requests.load(Ordering::SeqCst),
        2,
        "the answer goes back to the model as a tool message"
    );
    assert!(
        request(&fixture, 1).contains("\"tool_call_id\":\"call_q1\""),
        "the answer is addressed to the question's call"
    );
    assert!(
        said(&app).contains("Going with b."),
        "the transcript ends on the model's answer: {:?}",
        said(&app)
    );
}

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
    assert!(
        session
            .transcript()
            .items()
            .iter()
            .any(|item| matches!(item, Item::Assistant { text } if text.to_lowercase().contains("cancelled"))),
        "the transcript says the turn was cancelled: {:?}",
        session.transcript().items()
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
