//! What a finished, failed, cancelled and silent delegation answers with.

use std::sync::Arc;

use perch_agent::cancel::Cancel;
use perch_tools::Tool;

use crate::harness::{self, Memory};

#[test]
fn the_result_names_the_task_above_the_childs_answer() {
    let (task, memory) = harness::tool(
        &harness::parent(),
        Memory::ok("retries stop at three").in_rounds(2),
    );
    let out = task
        .call(
            &harness::args("read the retry policy", "find the retry policy"),
            &harness::ctx(),
        )
        .expect("a child that answered is a result");
    assert_eq!(
        out.text,
        "task \"read the retry policy\" -> done in 2 rounds\n\nretries stop at three"
    );
    assert_eq!(memory.calls(), 1, "the child ran more than once");
}

#[test]
fn a_child_that_counts_one_round_says_round_not_rounds() {
    let (task, _) = harness::tool(&harness::parent(), Memory::ok("done").in_rounds(1));
    let out = task
        .call(&harness::args("read one file", "read it"), &harness::ctx())
        .expect("a child that answered is a result");
    assert!(
        out.text
            .starts_with("task \"read one file\" -> done in 1 round\n"),
        "{}",
        out.text
    );
}

#[test]
fn a_delegate_error_is_a_tool_error_that_names_it() {
    let (task, _) = harness::tool(&harness::parent(), Memory::err("the provider answered 500"));
    let why = harness::refused(task.call(
        &harness::args("read the retry policy", "find it"),
        &harness::ctx(),
    ));
    assert!(why.contains("the provider answered 500"), "{why}");
    assert!(why.contains("task"), "{why}");
    assert!(
        why.contains("read the retry policy"),
        "the failure does not name the task: {why}"
    );
}

#[test]
fn a_cancelled_child_is_reported_as_cancelled_and_never_as_an_answer() {
    let cancel = Arc::new(Cancel::default());
    let (task, _) = harness::tool(
        &harness::parent(),
        Memory::ok("half an answer").cancelling(),
    );
    let task = task.cancelled_by(cancel);
    let why = harness::refused(task.call(
        &harness::args("read the retry policy", "find it"),
        &harness::ctx(),
    ));
    assert!(why.contains("cancelled"), "{why}");
    assert!(
        !why.contains("half an answer"),
        "a partial answer was returned as though it were complete: {why}"
    );
}

#[test]
fn a_child_that_says_nothing_is_not_an_empty_success() {
    let (task, _) = harness::tool(&harness::parent(), Memory::ok("   \n"));
    let why = harness::refused(task.call(
        &harness::args("read the retry policy", "find it"),
        &harness::ctx(),
    ));
    assert!(why.contains("without an answer"), "{why}");
}
