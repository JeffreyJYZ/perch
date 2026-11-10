//! The question tool: an ordinary tool whose answer is the person's, not the dispatcher's.

use serde_json::json;

use perch_agent::dispatch::{Answer, Dispatcher};

use crate::harness::{Script, call, ctx, registry};

#[test]
fn the_question_tool_returns_the_picked_option_and_free_text() {
    let dir = tempfile::tempdir().unwrap();
    let root = ctx(dir.path());
    let mut dispatcher = Dispatcher::new();
    let registry = registry();

    // `ask_user` is an ordinary tool: registered with a schema, and never gated by approval.
    let names: Vec<String> = registry.specs().into_iter().map(|spec| spec.name).collect();
    assert!(
        names.contains(&"ask_user".to_string()),
        "ask_user must be registered like any other tool, got {names:?}"
    );
    assert!(!registry.needs_approval("ask_user").unwrap());

    let mut interactor = Script::answer(Answer::Picks(vec![1]));
    let out = dispatcher.run(
        &[call(
            "q1",
            "ask_user",
            json!({"prompt": "which?", "options": ["a", "b"]}),
        )],
        &registry,
        &root,
        &mut interactor,
    );
    assert_eq!(out[0].message.tool_call_id.as_deref(), Some("q1"));
    assert_eq!(out[0].message.content.as_deref(), Some("b"));

    let mut interactor = Script::answer(Answer::Text("neither, use c".into()));
    let out = dispatcher.run(
        &[call(
            "q2",
            "ask_user",
            json!({"prompt": "which?", "options": ["a", "b"]}),
        )],
        &registry,
        &root,
        &mut interactor,
    );
    assert_eq!(out[0].message.tool_call_id.as_deref(), Some("q2"));
    assert_eq!(out[0].message.content.as_deref(), Some("neither, use c"));
}

#[test]
fn cancelling_a_question_ends_the_turn_rather_than_answering_for_the_user() {
    let dir = tempfile::tempdir().unwrap();
    let root = ctx(dir.path());
    let mut dispatcher = Dispatcher::new();
    let mut interactor = Script::answer(Answer::Cancelled);
    let out = dispatcher.run(
        &[call("q1", "ask_user", json!({"prompt": "which?"}))],
        &registry(),
        &root,
        &mut interactor,
    );

    assert_eq!(out[0].message.tool_call_id.as_deref(), Some("q1"));
    assert_eq!(
        out[0].message.content.as_deref(),
        Some("the user cancelled the question")
    );
}

#[test]
fn a_two_option_pick_returns_both_texts_in_the_given_order() {
    let dir = tempfile::tempdir().unwrap();
    let root = ctx(dir.path());
    let mut dispatcher = Dispatcher::new();
    let registry = registry();

    let mut interactor = Script::answer(Answer::Picks(vec![1, 0]));
    let out = dispatcher.run(
        &[call(
            "m1",
            "ask_user",
            json!({"prompt": "which?", "options": ["a", "b"], "multiple": true}),
        )],
        &registry,
        &root,
        &mut interactor,
    );
    assert_eq!(out[0].message.tool_call_id.as_deref(), Some("m1"));
    assert_eq!(
        out[0].message.content.as_deref(),
        Some("b\na"),
        "a multi-pick answers in the order the user picked, one option per line"
    );

    let mut interactor = Script::answer(Answer::Picks(Vec::new()));
    let out = dispatcher.run(
        &[call(
            "m2",
            "ask_user",
            json!({"prompt": "which?", "options": ["a", "b"], "multiple": true}),
        )],
        &registry,
        &root,
        &mut interactor,
    );
    assert_eq!(
        out[0].message.content.as_deref(),
        Some("the user cancelled the question"),
        "picking nothing is a cancellation, not an empty answer"
    );
}
