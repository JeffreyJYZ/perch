//! One tool instance, many sessions: the name is read when a call runs, so a registry built once
//! per process follows whichever session the app has active.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use perch_board::{Slot, Store, tools_dynamic, tools_dynamic_with_cancel};
use perch_tools::{Tool, ToolCtx};
use serde_json::json;

fn ctx(root: &Path) -> ToolCtx {
    ToolCtx {
        root: root.to_path_buf(),
    }
}

fn board(root: &Path, slot: Arc<Slot>) -> Arc<dyn Tool> {
    tools_dynamic(root, slot)
        .into_iter()
        .find(|tool| tool.spec().name == "board")
        .expect("board is registered")
}

#[test]
fn one_tool_instance_posts_and_reads_as_whatever_the_slot_holds() {
    let project = tempfile::tempdir().expect("tempdir");
    let root = project.path().to_path_buf();
    let slot = Arc::new(Slot::new("lead"));
    let tool = board(&root, Arc::clone(&slot));

    assert_eq!(
        tool.call(&json!({}), &ctx(&root)).expect("read").text,
        "nothing new\n"
    );
    assert!(
        tool.call(&json!({ "post": "lead was here" }), &ctx(&root))
            .expect("post")
            .text
            .starts_with("posted #")
    );
    assert!(
        tool.call(&json!({}), &ctx(&root))
            .expect("read")
            .text
            .contains("lead was here")
    );
    assert_eq!(
        tool.call(&json!({}), &ctx(&root)).expect("read").text,
        "nothing new\n",
        "the lead's cursor moved"
    );

    // The same instance, one process, a different session.
    slot.set("worker");
    let again = tool.call(&json!({}), &ctx(&root)).expect("read").text;
    assert!(
        again.contains("lead was here"),
        "the worker has its own cursor, so the lead's post is new to it: {again}"
    );
    tool.call(&json!({ "post": "worker was here" }), &ctx(&root))
        .expect("post");
    let own = tool.call(&json!({}), &ctx(&root)).expect("read").text;
    assert!(own.contains("worker was here"), "{own}");
    assert_eq!(
        tool.call(&json!({}), &ctx(&root)).expect("read").text,
        "nothing new\n"
    );
}

#[test]
fn the_two_posts_carry_the_two_names_the_slot_held() {
    let project = tempfile::tempdir().expect("tempdir");
    let root = project.path().to_path_buf();
    let slot = Arc::new(Slot::new("lead"));
    let tool = board(&root, Arc::clone(&slot));

    tool.call(&json!({ "post": "from the lead" }), &ctx(&root))
        .expect("post");
    tool.call(&json!({}), &ctx(&root)).expect("read");
    slot.set("worker");
    tool.call(&json!({ "post": "from the worker" }), &ctx(&root))
        .expect("post");
    tool.call(&json!({}), &ctx(&root)).expect("read");

    let log = Store::new(&root, &root, "unused").log();
    let text = fs::read_to_string(log).expect("log");
    let entries: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).expect("entry"))
        .collect();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["from"], "lead");
    assert_eq!(entries[1]["from"], "worker");

    let cursors = fs::read_dir(root.join("board"))
        .expect("board dir")
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".cursor"))
        .count();
    assert_eq!(cursors, 2, "one cursor per session name");
}

#[test]
fn the_dynamic_pair_is_the_same_two_tools() {
    let project = tempfile::tempdir().expect("tempdir");
    let root = project.path().to_path_buf();
    let slot = Arc::new(Slot::new("lead"));
    let names: Vec<String> =
        tools_dynamic_with_cancel(&root, slot, Arc::new(perch_board::NeverStopped))
            .iter()
            .map(|tool| tool.spec().name)
            .collect();
    assert_eq!(names, vec!["board", "board_wait"]);
}
