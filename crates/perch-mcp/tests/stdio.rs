//! A stdio MCP server contributes tools the registry can dispatch, and a server that dies
//! takes its tools with it.

mod support;

use perch_mcp::McpHost;
use perch_tools::{ToolCtx, ToolRegistry};
use serde_json::json;
use support::{fixture_server, fixture_server_named};
use tempfile::tempdir;

#[test]
fn a_stdio_server_contributes_namespaced_tools() {
    let mut registry = ToolRegistry::builtin();
    let host = McpHost::connect_all(&[fixture_server()], &mut registry).unwrap();

    let names: Vec<String> = registry.specs().into_iter().map(|s| s.name).collect();
    assert!(
        names.contains(&"fixture__echo".to_string()),
        "expected fixture__echo among {names:?}"
    );

    let ctx = ToolCtx {
        root: tempdir().unwrap().path().to_path_buf(),
    };
    assert_eq!(
        registry
            .call("fixture__echo", &json!({"text": "hi"}), &ctx)
            .unwrap()
            .text,
        "hi"
    );

    // The user's call: an MCP tool runs without an approval prompt, unlike write/edit/bash.
    assert!(
        !registry.needs_approval("fixture__echo").unwrap(),
        "an MCP tool must not ask for approval"
    );

    // Every call is recorded with the server that served it, so the transcript can show it.
    let notes = host.note_lines();
    assert!(
        notes
            .iter()
            .any(|line| line.contains("fixture") && line.contains("echo")),
        "no call note naming the server: {notes:?}"
    );
    assert_eq!(host.server_names(), vec!["fixture".to_string()]);
}

#[test]
fn a_server_that_dies_is_reported_and_its_tools_are_dropped() {
    let mut registry = ToolRegistry::builtin();
    let host = McpHost::connect_all(&[fixture_server()], &mut registry).unwrap();
    assert!(registry.needs_approval("fixture__echo").is_ok());

    host.kill_for_test("fixture");
    assert!(
        !host.is_alive("fixture"),
        "the killed server is still reported alive"
    );
    assert!(host.disconnect(&mut registry) > 0);
    assert!(
        !registry
            .specs()
            .iter()
            .any(|s| s.name.starts_with("fixture__"))
    );
    assert!(host.server_names().is_empty());
    assert!(
        host.note_lines()
            .iter()
            .any(|line| line.contains("fixture") && line.contains("removed")),
        "the death was not reported: {:?}",
        host.note_lines()
    );
}

#[test]
fn namespacing_prevents_a_server_shadowing_a_builtin() {
    let mut registry = ToolRegistry::builtin();
    let _host = McpHost::connect_all(&[fixture_server_named("bash")], &mut registry).unwrap();

    let names: Vec<String> = registry.specs().into_iter().map(|s| s.name).collect();
    assert!(names.contains(&"bash".to_string()), "{names:?}");
    assert!(names.contains(&"bash__echo".to_string()), "{names:?}");
}
