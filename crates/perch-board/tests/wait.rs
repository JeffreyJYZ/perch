//! A wait is only useful if it returns the entry that arrives while it is parked, gives up with a
//! word rather than an error, and stops the moment the turn is cancelled.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use perch_board::{Post, Stopped, Store, tools_with_cancel};
use perch_tools::{Tool, ToolCtx};
use serde_json::json;

fn wait(root: &Path, session: &str, stopped: Arc<dyn Stopped>) -> Arc<dyn Tool> {
    tools_with_cancel(root, session, stopped)
        .into_iter()
        .find(|tool| tool.spec().name == "board_wait")
        .expect("board_wait is registered")
}

fn ctx(root: &Path) -> ToolCtx {
    ToolCtx {
        root: root.to_path_buf(),
    }
}

#[test]
fn a_wait_returns_the_entry_a_second_thread_posts_during_it() {
    let project = tempfile::tempdir().expect("tempdir");
    let root = project.path().to_path_buf();
    let worker = Store::new(&root, &root, "worker");
    let poster = thread::spawn(move || {
        thread::sleep(Duration::from_millis(60));
        worker
            .append(Post {
                text: "worker: wave 1 is done".to_string(),
                group: Some("wave-1".to_string()),
                ..Post::default()
            })
            .expect("append");
    });

    let started = Instant::now();
    let out = wait(&root, "lead", Arc::new(perch_board::NeverStopped))
        .call(
            &json!({ "match": "done", "timeout_ms": 5_000 }),
            &ctx(&root),
        )
        .expect("a match is not an error");
    poster.join().expect("post");

    assert!(out.text.contains("worker: wave 1 is done"), "{}", out.text);
    assert!(
        out.text.contains("wave-1"),
        "the group travels with the entry: {}",
        out.text
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "returned before the timeout"
    );
}

#[test]
fn a_wait_that_matches_nothing_says_it_timed_out_without_failing() {
    let project = tempfile::tempdir().expect("tempdir");
    let root = project.path().to_path_buf();

    let out = wait(&root, "lead", Arc::new(perch_board::NeverStopped))
        .call(
            &json!({ "match": "never posted", "timeout_ms": 120 }),
            &ctx(&root),
        )
        .expect("a timeout is an answer, not an error");
    assert!(out.text.contains("timed out"), "{}", out.text);
    assert!(
        out.text.contains("nothing new"),
        "it still reports what is new: {}",
        out.text
    );
}

#[test]
fn a_cancelled_turn_stops_the_wait_instead_of_parking_for_the_timeout() {
    let project = tempfile::tempdir().expect("tempdir");
    let root = project.path().to_path_buf();
    let flag = Arc::new(AtomicBool::new(false));
    let cancel = Arc::clone(&flag);
    let cancelled: Arc<dyn Stopped> = cancel;
    let flipper = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        flag.store(true, Ordering::SeqCst);
    });

    let started = Instant::now();
    let result = wait(&root, "lead", cancelled).call(
        &json!({ "match": "never posted", "timeout_ms": 30_000 }),
        &ctx(&root),
    );
    flipper.join().expect("cancel");

    let error = result.expect_err("a cancelled wait ends in an error, and early");
    assert!(error.to_string().contains("cancelled"), "{error}");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "stopped at the next poll, took {:?}",
        started.elapsed()
    );
}
