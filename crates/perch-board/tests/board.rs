//! What a session sees, and what it does not: the read-once rule, two projects kept apart, a log
//! with a broken line in it, and a read that has nothing to say.

use std::fs;
use std::path::Path;

use perch_board::{Post, Read, Store, tools};
use perch_tools::ToolCtx;
use serde_json::json;

fn post(store: &Store, text: &str) -> u64 {
    let entry = store
        .append(Post {
            text: text.to_string(),
            ..Post::default()
        })
        .expect("append");
    entry.id
}

fn texts(read: &Read) -> Vec<String> {
    read.entries
        .iter()
        .map(|entry| entry.text.clone())
        .collect()
}

fn read_with(root: &Path, session: &str) -> String {
    let tool = tools(root, session)
        .into_iter()
        .find(|tool| tool.spec().name == "board")
        .expect("board is registered");
    let ctx = ToolCtx {
        root: root.to_path_buf(),
    };
    tool.call(&json!({}), &ctx).expect("read").text
}

#[test]
fn a_post_is_unseen_once_for_the_poster_and_once_for_a_second_session() {
    let project = tempfile::tempdir().expect("tempdir");
    let root = project.path();
    let poster = Store::new(root, root, "lead");
    let id = post(&poster, "wave 1: the schema is done");

    let mine = poster.take().expect("read");
    assert_eq!(texts(&mine), vec!["wave 1: the schema is done"]);
    assert_eq!(
        mine.entries[0].id, id,
        "the post answered the id it was given"
    );
    assert_eq!(
        mine.entries[0].from, "lead",
        "an entry carries who posted it"
    );
    assert!(
        poster.take().expect("read").entries.is_empty(),
        "read once only"
    );

    let worker = Store::new(root, root, "worker");
    assert_eq!(
        texts(&worker.take().expect("read")),
        vec!["wave 1: the schema is done"]
    );
    assert!(
        worker.take().expect("read").entries.is_empty(),
        "read once only"
    );
}

#[test]
fn a_read_with_nothing_new_says_so_in_one_line() {
    let project = tempfile::tempdir().expect("tempdir");
    let root = project.path();
    assert_eq!(
        read_with(root, "lead"),
        "nothing new\n",
        "a blank board is not an error"
    );

    post(&Store::new(root, root, "worker"), "worker: queued");
    let first = read_with(root, "lead");
    assert!(first.contains("worker: queued"), "{first}");
    assert_eq!(
        read_with(root, "lead"),
        "nothing new\n",
        "the second read is empty again"
    );
}

#[test]
fn two_projects_under_one_root_never_see_each_other() {
    let shared = tempfile::tempdir().expect("tempdir");
    let one = shared.path().join("one");
    let two = shared.path().join("two");
    fs::create_dir(&one).expect("one");
    fs::create_dir(&two).expect("two");

    let first = Store::new(shared.path(), &one, "lead");
    let second = Store::new(shared.path(), &two, "lead");
    post(&first, "only the first project sees this");
    post(&second, "only the second project sees this");

    assert_ne!(first.log(), second.log(), "a board file per project");
    assert_eq!(
        texts(&first.take().expect("read")),
        vec!["only the first project sees this"]
    );
    assert_eq!(
        texts(&second.take().expect("read")),
        vec!["only the second project sees this"]
    );
}

#[test]
fn a_malformed_line_is_skipped_and_the_rest_still_reads() {
    let project = tempfile::tempdir().expect("tempdir");
    let root = project.path();
    let poster = Store::new(root, root, "lead");
    post(&poster, "before the damage");

    let log = poster.log();
    let mut bytes = fs::read_to_string(&log).expect("log");
    bytes.push_str("{ this is not an entry\n");
    bytes.push('\n');
    bytes.push_str("{\"id\": \"not a number\"}\n");
    fs::write(&log, bytes).expect("write");

    post(&poster, "after the damage");
    let read = Store::new(root, root, "worker").take().expect("read");
    assert_eq!(texts(&read), vec!["before the damage", "after the damage"]);
}

#[test]
fn a_cursor_survives_a_restart_and_is_per_session() {
    let project = tempfile::tempdir().expect("tempdir");
    let root = project.path();
    let id = post(&Store::new(root, root, "lead"), "one line, read once");
    let restarted = Store::new(root, root, "lead").take().expect("read");
    assert_eq!(restarted.entries.len(), 1);
    assert!(
        Store::new(root, root, "lead")
            .take()
            .expect("read")
            .entries
            .is_empty()
    );

    let fresh = tempfile::tempdir().expect("tempdir");
    let other = fresh.path();
    Store::new(other, other, "lead")
        .append(Post::default())
        .expect("append");
    assert_eq!(
        Store::new(other, other, "lead")
            .take()
            .expect("read")
            .entries
            .len(),
        1
    );
    assert!(id > 0);
}
