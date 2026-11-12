//! What one execution may spend, and how each stop reads: a plain thing to tell the model.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{field, result};
use perch_codemode::{Execute, Limits, MemoryHost, Watch};
use serde_json::json;

#[test]
fn a_syntax_error_carries_its_line_and_column() {
    let tool = Execute::new(Arc::new(MemoryHost::new()));

    let answer = result(&tool, "const first = 1;\nconst second = ;\n");

    assert_eq!(answer["ok"], json!(false));
    assert!(
        field(&answer, "error").contains("line 2"),
        "{}",
        field(&answer, "error")
    );
    let diagnostic = field(&answer, "diagnostic");
    assert!(diagnostic.contains("line 2, column"), "{diagnostic}");
    assert!(diagnostic.contains('^'), "the caret is drawn: {diagnostic}");
}

#[test]
fn the_output_cap_truncates_and_says_so() {
    let tool = Execute::new(Arc::new(MemoryHost::new())).accepting(Limits {
        output: 64,
        ..Limits::default()
    });

    let answer = result(
        &tool,
        r#"
for (let i = 0; i < 100; i++) {
    print("0123456789");
}
"#,
    );

    assert_eq!(answer["ok"], json!(false));
    assert!(
        field(&answer, "error").contains("output budget"),
        "{}",
        field(&answer, "error")
    );
    assert!(
        field(&answer, "error").contains("truncated"),
        "{}",
        field(&answer, "error")
    );
    let diagnostic = field(&answer, "diagnostic");
    assert!(!diagnostic.is_empty(), "what was printed is kept");
    assert!(diagnostic.len() <= 64, "{} bytes", diagnostic.len());
}

#[test]
fn the_tool_call_budget_stops_at_32_and_says_so() {
    let host = Arc::new(MemoryHost::new().answering("read", "same"));
    let tool = Execute::new(host.clone());

    let answer = result(
        &tool,
        r#"
for (let i = 0; i < 40; i++) {
    tools.call("read", { path: "same.txt" });
}
print("never printed");
"#,
    );

    assert_eq!(answer["ok"], json!(false));
    assert!(
        field(&answer, "error").contains("32"),
        "{}",
        field(&answer, "error")
    );
    assert_eq!(host.count(), 32, "the call past the budget never ran");
    assert_eq!(field(&answer, "output"), "");
}

/// The one test that proves the loop allowance is real. It runs the call on its own thread and
/// waits with a hard timeout, so a wrong mechanism fails loudly here instead of hanging the suite.
#[test]
fn a_loop_that_never_returns_is_stopped_by_the_clock() {
    let tool = Execute::new(Arc::new(MemoryHost::new()));
    let (send, receive) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let _ = send.send(result(&tool, "while (true) {}"));
    });

    let answer = receive
        .recv_timeout(Duration::from_secs(120))
        .expect("the loop allowance did not stop the program: it is still spinning");

    assert_eq!(answer["ok"], json!(false));
    assert!(
        field(&answer, "error").contains("wall-clock"),
        "{}",
        field(&answer, "error")
    );
    worker.join().expect("the worker thread finished");
}

#[test]
fn a_stopped_turn_stops_the_program() {
    let watch = Watch::new();
    watch.stop();
    let tool = Execute::new(Arc::new(MemoryHost::new())).cancelled_by(Arc::new(watch));

    let answer = result(&tool, r#"print("started");"#);

    assert_eq!(answer["ok"], json!(false));
    assert!(
        field(&answer, "error").contains("cancelled"),
        "{}",
        field(&answer, "error")
    );
}
