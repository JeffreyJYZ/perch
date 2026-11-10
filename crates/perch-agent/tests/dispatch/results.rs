//! What comes back when a call fails: message content, never a panic.

use serde_json::json;

use perch_agent::dispatch::{Answer, Dispatcher};

use crate::harness::{Script, call, ctx, registry};

#[test]
fn a_failing_tool_becomes_message_content_not_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    let root = ctx(dir.path());
    let mut dispatcher = Dispatcher::new();
    let mut interactor = Script::answer(Answer::Cancelled);
    let out = dispatcher.run(
        &[
            call("c1", "read", json!({"path": "missing.txt"})),
            call("c2", "no_such_tool", json!({})),
        ],
        &registry(),
        &root,
        &mut interactor,
    );

    assert_eq!(out.len(), 2, "one tool message per call, whatever happened");
    assert_eq!(out[0].message.tool_call_id.as_deref(), Some("c1"));
    assert_eq!(out[1].message.tool_call_id.as_deref(), Some("c2"));
    assert!(
        out[0]
            .message
            .content
            .as_deref()
            .unwrap()
            .contains("No such file or directory"),
        "the read failure reaches the model as text, got {:?}",
        out[0].message.content
    );
    assert!(
        out[1]
            .message
            .content
            .as_deref()
            .unwrap()
            .contains("no tool named"),
        "an unknown tool is reported, not panicked on, got {:?}",
        out[1].message.content
    );
}

#[test]
fn a_command_that_exits_non_zero_still_ran() {
    let dir = tempfile::tempdir().unwrap();
    let root = ctx(dir.path());
    let mut dispatcher = Dispatcher::new();
    let mut interactor = Script::answer(Answer::Cancelled);
    let out = dispatcher.run(
        &[call("b1", "bash", json!({"command": "exit 3"}))],
        &registry(),
        &root,
        &mut interactor,
    );

    assert!(
        out[0].ok,
        "a failing command is a result the model must see, not a failed call: {:?}",
        out[0].message.content
    );
    assert_eq!(out[0].message.content.as_deref(), Some("exit 3"));
}
