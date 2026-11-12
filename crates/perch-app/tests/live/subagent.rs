//! The `task` tool, end to end: a session delegates one turn, and the child's answer comes back.
//!
//! The promise has three halves and each is read from the wire rather than from a struct: the tool
//! result carries the child's text, the child asked the provider with only the tools that need no
//! approval, and a cancel of the parent's turn stops the child instead of leaving it working.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use perch_app::{AppState, SessionRuntime};
use perch_ui::{Item, ToolStatus};
use serde_json::Value;

use crate::harness::{SETTLE, TIMEOUT, app_against, request, turn_requests};

/// The tool's result as the card shows it, empty while it is still running.
fn task_output(session: &SessionRuntime) -> String {
    session
        .transcript()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::Tool { name, output, .. } if name == "task" => Some(output.clone()),
            _ => None,
        })
        .collect()
}

/// Waits until the session is offered `task`, which is the registry the worker publishes.
///
/// The window draws with the built-ins first — `task` lives with the sources, because a child
/// needs a provider the built-ins do not have — so a turn started too early would be answered
/// `unknown tool`.
fn wait_for_task(app: &mut AppState) {
    let deadline = Instant::now() + TIMEOUT;
    while !app.tools().iter().any(|spec| spec.name == "task") {
        assert!(
            Instant::now() < deadline,
            "the tools never offered `task`: {:?}",
            app.tools()
        );
        app.pump(Duration::from_millis(20));
    }
}

/// The body of request `index` as JSON, in arrival order: the parent's, the child's, the parent's.
fn body(fixture: &crate::support::Fixture, index: usize) -> Value {
    serde_json::from_str(&request(fixture, index)).expect("a request body is JSON")
}

/// The tool names one request offered the model.
fn offered(body: &Value) -> Vec<String> {
    body["tools"]
        .as_array()
        .expect("a request carries a tools array")
        .iter()
        .filter_map(|tool| tool["function"]["name"].as_str().map(str::to_string))
        .collect()
}

#[test]
fn a_session_calls_task_and_reads_the_childs_answer() {
    let (mut app, fixture, _data, _worktree) = app_against("tests/fixtures/live_task.txt");
    wait_for_task(&mut app);

    app.prompt("where does the retry policy live?").unwrap();
    assert!(
        app.pump_until(TIMEOUT, |session| session.turns_ended() == 1),
        "the turn should end within {TIMEOUT:?}"
    );

    let output = task_output(app.active_session().unwrap());
    assert!(
        output.contains("the retry policy lives in src/retry.rs"),
        "the tool result does not carry the child's answer: {output}"
    );
    assert!(
        output.contains("done in 1 round"),
        "the result line does not say what the child cost: {output}"
    );
    let card = app
        .active_session()
        .unwrap()
        .transcript()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::Tool { name, status, .. } if name == "task" => Some(*status),
            _ => None,
        })
        .expect("the call is a card in the transcript");
    assert_eq!(card, ToolStatus::Ok, "{output}");

    // Three requests: the parent's, the child's, and the parent's answer to the result.
    assert_eq!(
        turn_requests(&fixture),
        3,
        "the child runs its own conversation and the parent is asked again with the answer"
    );
    let child = body(&fixture, 1);
    let system = child["messages"][0]["content"]
        .as_str()
        .expect("the child's first message is its system message");
    assert!(
        system.contains("child agent"),
        "the child is told what it is: {system}"
    );

    // The child's registry, read off the wire: everything that asks a person is gone, and `task`
    // is gone too, so a child can never have one of its own.
    let offered = offered(&child);
    for gone in ["write", "edit", "bash", "patch", "task", "execute"] {
        assert!(
            !offered.iter().any(|name| name == gone),
            "the child was offered `{gone}`: {offered:?}"
        );
    }
    assert!(
        offered.iter().any(|name| name == "read"),
        "the child lost `read`, which asks nobody anything: {offered:?}"
    );
}

#[test]
fn a_cancelled_turn_cancels_the_child_and_nothing_hangs() {
    let (mut app, fixture, _data, _worktree) = app_against("tests/fixtures/live_task_cancel.txt");
    wait_for_task(&mut app);

    app.prompt("look it up").unwrap();
    // The child's own request is the second one: until it has arrived there is nothing running to
    // cancel, and the test would be proving the wrong turn stopped.
    let deadline = Instant::now() + TIMEOUT;
    while fixture.requests.load(Ordering::SeqCst) < 2 {
        assert!(
            Instant::now() < deadline,
            "the child never asked the provider: {:?}",
            fixture.bodies.lock().unwrap()
        );
        app.pump(Duration::from_millis(20));
    }

    app.cancel();
    assert!(
        app.pump_until(TIMEOUT, |session| session.turns_ended() == 1),
        "a cancel still ends the parent turn"
    );
    // The tool was running when the press landed, so its result arrives after the ending.
    assert!(
        app.pump_until(TIMEOUT, |session| task_output(session)
            .contains("cancelled")),
        "the tool result does not say the child was cancelled: {:?}",
        task_output(app.active_session().unwrap())
    );
    app.pump_until(SETTLE, |_| false);

    let session = app.active_session().unwrap();
    assert_eq!(
        session.turns_ended(),
        1,
        "a cancelled turn ends exactly once"
    );
}
