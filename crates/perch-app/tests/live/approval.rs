//! A call that needs approval: allowed, denied, and cancelled while it waits.

use std::sync::atomic::Ordering;

use perch_agent::Decision;
use perch_store::SessionEvent;
use perch_ui::{Item, ToolStatus};

use crate::harness::{SETTLE, TIMEOUT, app_against, request, said, tool_card};

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
