//! Approval: what a refusal and an allow-for-session do to the call, and how `ok` reads.

use serde_json::json;

use perch_agent::dispatch::{Answer, Dispatcher};

use crate::harness::{Script, call, ctx, registry};

#[test]
fn a_denied_tool_never_runs_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let root = ctx(dir.path());
    let mut dispatcher = Dispatcher::new();
    let mut interactor = Script::deny_all();
    let out = dispatcher.run(
        &[call("call_1", "bash", json!({"command": "touch pwned"}))],
        &registry(),
        &root,
        &mut interactor,
    );

    assert_eq!(out.len(), 1);
    assert_eq!(out[0].message.tool_call_id.as_deref(), Some("call_1"));
    let text = out[0].message.content.clone().unwrap();
    assert!(
        text.contains("denied"),
        "the model must be told the call was denied, got: {text}"
    );
    assert!(
        !dir.path().join("pwned").exists(),
        "a denied call must not have run"
    );
    assert_eq!(
        interactor.previews,
        vec![Some(perch_tools::Preview::Command(
            "touch pwned".to_string()
        ))],
        "the approval view is shown the command before anything runs"
    );
}

#[test]
fn allow_for_session_stops_asking_for_that_tool() {
    let dir = tempfile::tempdir().unwrap();
    let root = ctx(dir.path());
    let mut dispatcher = Dispatcher::new();
    let mut interactor = Script::allow_session_once();

    for (index, id) in ["c1", "c2", "c3"].iter().enumerate() {
        let out = dispatcher.run(
            &[call(id, "bash", json!({"command": "printf ok"}))],
            &registry(),
            &root,
            &mut interactor,
        );
        assert_eq!(out[0].message.tool_call_id.as_deref(), Some(*id));
        assert!(
            out[0].message.content.as_deref().unwrap().contains("ok"),
            "run {index} was not allowed to run: {:?}",
            out[0].message.content
        );
    }

    assert_eq!(
        interactor.prompts, 1,
        "allowing a tool for the session must stop the later prompts"
    );
}

#[test]
fn a_denied_call_is_not_ok_and_a_call_that_ran_is() {
    let dir = tempfile::tempdir().unwrap();
    let root = ctx(dir.path());
    let mut dispatcher = Dispatcher::new();

    let mut interactor = Script::deny_all();
    let out = dispatcher.run(
        &[call("d1", "bash", json!({"command": "touch pwned"}))],
        &registry(),
        &root,
        &mut interactor,
    );
    assert!(!out[0].ok, "a denied call must be reported as not run");
    assert!(!dir.path().join("pwned").exists(), "and it must not run");

    let mut interactor = Script::answer(Answer::Cancelled);
    let out = dispatcher.run(
        &[call("r1", "read", json!({"path": "missing.txt"}))],
        &registry(),
        &root,
        &mut interactor,
    );
    assert!(!out[0].ok, "a tool that returned an error is not ok");

    let path = dir.path().join("a.txt");
    std::fs::write(&path, "hello\n").unwrap();
    let mut interactor = Script::answer(Answer::Cancelled);
    let out = dispatcher.run(
        &[call("r2", "read", json!({"path": "a.txt"}))],
        &registry(),
        &root,
        &mut interactor,
    );
    assert!(
        out[0].ok,
        "a tool that ran is ok: {:?}",
        out[0].message.content
    );
    assert!(
        out[0].message.content.as_deref().unwrap().contains("hello"),
        "the file's text reaches the model, got {:?}",
        out[0].message.content
    );
}
