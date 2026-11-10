//! A question: it waits for the person, and the pick goes back to the model.

use std::sync::atomic::Ordering;

use perch_agent::Answer;
use perch_app::Pending;
use perch_ui::Item;

use crate::harness::{TIMEOUT, app_against, request, said, tool_card};

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
