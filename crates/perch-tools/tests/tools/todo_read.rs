//! Reading the plan: what an empty one says, what the model is told about the tools, and that two
//! worktrees never share one list.

use perch_tools::ToolRegistry;
use serde_json::{Value, json};

use crate::support::root_of;
use crate::todo::{item, read, write};

#[test]
fn reading_a_fresh_worktree_says_there_are_none() {
    let root = tempfile::tempdir().unwrap();
    let ctx = root_of(&root);
    let registry = ToolRegistry::builtin();

    let text = registry.call("todoread", &json!({}), &ctx).unwrap().text;
    assert!(
        text.contains("no todos"),
        "a fresh worktree does not say the list is empty: {text}"
    );
    assert!(
        serde_json::from_str::<Value>(&text).is_err(),
        "the empty answer reads as a list rather than as words: {text}"
    );
}

#[test]
fn both_tools_say_when_to_use_them_and_that_one_step_is_in_progress() {
    let registry = ToolRegistry::builtin();
    let specs = registry.specs();
    let write = specs
        .iter()
        .find(|spec| spec.name == "todowrite")
        .expect("todowrite is offered");
    for word in [
        "multi-step",
        "after each step finishes",
        "replaces",
        "in_progress",
    ] {
        assert!(
            write.description.contains(word),
            "the description does not say {word}: {}",
            write.description
        );
    }
    assert_eq!(
        write.parameters["properties"]["todos"]["items"]["properties"]["status"]["enum"],
        json!(["pending", "in_progress", "completed"])
    );

    let read = specs
        .iter()
        .find(|spec| spec.name == "todoread")
        .expect("todoread is offered");
    assert!(
        read.description.contains("todowrite") && read.description.contains("none"),
        "the description does not say what it reads: {}",
        read.description
    );

    for name in ["todowrite", "todoread"] {
        assert!(
            !registry.needs_approval(name).unwrap(),
            "{name} asks for approval although it changes nothing on disk"
        );
    }
}

#[test]
fn two_worktree_roots_never_see_each_others_lists() {
    let one = tempfile::tempdir().unwrap();
    let two = tempfile::tempdir().unwrap();
    let registry = ToolRegistry::builtin();

    write(
        &registry,
        &root_of(&one),
        json!([item("only in one", "pending")]),
    );

    let other = registry
        .call("todoread", &json!({}), &root_of(&two))
        .unwrap()
        .text;
    assert!(
        other.contains("no todos"),
        "a second worktree saw the first one's plan: {other}"
    );

    let mine = read(&registry, &root_of(&one));
    assert_eq!(mine.len(), 1, "{mine:?}");
    assert_eq!(mine[0].content, "only in one");
}
