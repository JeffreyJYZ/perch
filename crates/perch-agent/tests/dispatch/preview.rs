//! What the approval view is shown: the change a call will make, computed and never written.

use serde_json::json;

use perch_agent::dispatch::{Answer, Dispatcher};
use perch_tools::Preview;

use crate::harness::{Script, call, ctx, registry};

#[test]
fn denying_an_edit_shows_the_change_and_leaves_the_file_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let root = ctx(dir.path());
    let path = dir.path().join("a.rs");
    std::fs::write(&path, "fn alpha() {}\n").unwrap();
    let mut dispatcher = Dispatcher::new();
    let mut interactor = Script::deny_all();
    let out = dispatcher.run(
        &[call(
            "e1",
            "edit",
            json!({"path": "a.rs", "old": "alpha", "new": "beta"}),
        )],
        &registry(),
        &root,
        &mut interactor,
    );

    let Some(Preview::Diff(diff)) = interactor.previews[0].clone() else {
        panic!(
            "an edit must preview its diff, got {:?}",
            interactor.previews[0]
        );
    };
    assert!(
        diff.contains("-fn alpha() {}"),
        "the removed line must be in the preview, got:\n{diff}"
    );
    assert!(
        diff.contains("+fn beta() {}"),
        "the added line must be in the preview, got:\n{diff}"
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        b"fn alpha() {}\n",
        "a preview must not write, and a denied edit must not either"
    );
    assert_eq!(out[0].message.tool_call_id.as_deref(), Some("e1"));
    assert!(
        out[0]
            .message
            .content
            .as_deref()
            .unwrap()
            .contains("denied"),
        "got {:?}",
        out[0].message.content
    );
}

#[test]
fn denying_a_write_over_an_existing_file_shows_a_diff() {
    let dir = tempfile::tempdir().unwrap();
    let root = ctx(dir.path());
    let path = dir.path().join("notes.txt");
    std::fs::write(&path, "old line\n").unwrap();
    let mut dispatcher = Dispatcher::new();
    let mut interactor = Script::deny_all();
    let out = dispatcher.run(
        &[call(
            "w1",
            "write",
            json!({"path": "notes.txt", "text": "new line\n"}),
        )],
        &registry(),
        &root,
        &mut interactor,
    );

    let Some(Preview::Diff(diff)) = interactor.previews[0].clone() else {
        panic!(
            "a write must preview its diff, got {:?}",
            interactor.previews[0]
        );
    };
    assert!(
        diff.contains("-old line"),
        "the replaced line must be in the preview, got:\n{diff}"
    );
    assert!(
        diff.contains("+new line"),
        "the new line must be in the preview, got:\n{diff}"
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        b"old line\n",
        "a denied write must not touch the file"
    );
    assert_eq!(out[0].message.tool_call_id.as_deref(), Some("w1"));
}

#[test]
fn a_preview_that_fails_denies_without_asking_or_panicking() {
    let dir = tempfile::tempdir().unwrap();
    let root = ctx(dir.path());
    let mut dispatcher = Dispatcher::new();
    let mut interactor = Script::answer(Answer::Picks(Vec::new()));
    let out = dispatcher.run(
        &[call(
            "w1",
            "write",
            json!({"path": "../escape.txt", "text": "x"}),
        )],
        &registry(),
        &root,
        &mut interactor,
    );

    assert_eq!(
        interactor.prompts, 0,
        "a change that cannot be computed is never offered to the user"
    );
    let text = out[0].message.content.clone().unwrap();
    assert!(
        text.contains("not run") && text.contains("outside the worktree root"),
        "the model must be told why the call did not run, got: {text}"
    );
    assert_eq!(out[0].message.tool_call_id.as_deref(), Some("w1"));
}
