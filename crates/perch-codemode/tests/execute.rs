//! What a JavaScript program can reach: the host's tools, the approval boundary, and failures.

mod common;

use std::sync::Arc;

use common::{field, result};
use perch_codemode::{Execute, MemoryHost};
use perch_tools::Tool;
use serde_json::json;

#[test]
fn a_program_that_reads_twice_is_one_result_from_two_calls() {
    let host = Arc::new(MemoryHost::new().answering("read", "alpha"));
    let tool = Execute::new(host.clone());
    assert!(
        !tool.needs_approval(),
        "a program cannot reach a gated tool, so execute itself needs no approval"
    );

    let answer = result(
        &tool,
        r#"
const first = tools.call("read", { path: "a.txt" });
const second = tools.call("read", { path: "b.txt" });
print(first.text + " then " + second.text);
"#,
    );

    assert_eq!(answer["ok"], json!(true));
    assert_eq!(field(&answer, "output"), "alpha then alpha\n");
    assert_eq!(host.count(), 2, "one program, two calls");
    assert_eq!(host.calls()[0].1, json!({ "path": "a.txt" }));
    assert_eq!(host.calls()[1].1, json!({ "path": "b.txt" }));
}

#[test]
fn print_writes_lines_and_takes_a_json_string_as_arguments() {
    let host = Arc::new(MemoryHost::new().answering("read", "alpha"));
    let tool = Execute::new(host.clone());

    let answer = result(
        &tool,
        r#"
print("values:", 1, true, null);
print(tools.call("read", '{"path":"c.txt"}').text);
print(tools.call("read", { path: "d.txt" }).text);
"#,
    );

    assert_eq!(answer["ok"], json!(true));
    assert_eq!(
        field(&answer, "output"),
        "values: 1 true null\nalpha\nalpha\n"
    );
    assert_eq!(host.calls()[0].1, json!({ "path": "c.txt" }));
    assert_eq!(host.calls()[1].1, json!({ "path": "d.txt" }));
}

#[test]
fn the_program_can_list_the_tools_it_may_call() {
    let host = Arc::new(MemoryHost::new().answering("read", "alpha").gated("write"));
    let tool = Execute::new(host);

    let answer = result(
        &tool,
        r#"
const names = tools.list().map((tool) => tool.name);
print(names.join(","));
"#,
    );

    assert_eq!(answer["ok"], json!(true));
    assert_eq!(field(&answer, "output"), "read,write\n");
}

#[test]
fn a_tool_that_needs_approval_is_refused_through_execute() {
    let host = Arc::new(
        MemoryHost::new()
            .answering("read", "alpha")
            .answering("write", "wrote it")
            .gated("write"),
    );
    let tool = Execute::new(host.clone());

    let answer = result(
        &tool,
        r#"
try {
    tools.call("write", { path: "a.txt", content: "b" });
    print("it ran");
} catch (error) {
    print("refused: " + error.message);
}
"#,
    );

    assert_eq!(answer["ok"], json!(true), "a refusal is not a crash");
    let text = field(&answer, "output");
    assert!(text.starts_with("refused: "), "{text}");
    assert!(text.contains("asks a person before it runs"), "{text}");
    assert!(text.contains("call `write` directly"), "{text}");
    assert_eq!(host.count(), 0, "the write never reached the host");
}

#[test]
fn a_thrown_error_the_program_catches_is_data() {
    let tool = Execute::new(Arc::new(MemoryHost::new()));

    let answer = result(
        &tool,
        r#"
try {
    throw new Error("nope");
} catch (error) {
    print("caught: " + error.message);
}
"#,
    );

    assert_eq!(answer["ok"], json!(true));
    assert_eq!(field(&answer, "output"), "caught: nope\n");
}

#[test]
fn an_uncaught_throw_ends_the_program_with_its_message() {
    let tool = Execute::new(Arc::new(MemoryHost::new()));

    let answer = result(&tool, "throw new Error('boom');");

    assert_eq!(answer["ok"], json!(false));
    assert!(
        field(&answer, "error").contains("boom"),
        "{}",
        field(&answer, "error")
    );
    assert!(
        !field(&answer, "diagnostic").is_empty(),
        "the stack or the message is reported"
    );
}

#[test]
fn the_last_expression_is_reported_when_nothing_printed() {
    let host = Arc::new(MemoryHost::new().answering("read", "alpha"));
    let tool = Execute::new(host);

    let sum = result(&tool, "1 + 2");
    assert_eq!(field(&sum, "output"), "3");

    let call = result(&tool, r#"tools.call("read", { path: "a.txt" })"#);
    assert_eq!(call["ok"], json!(true));
    assert!(
        field(&call, "output").contains("\"text\":\"alpha\""),
        "{}",
        field(&call, "output")
    );
}
