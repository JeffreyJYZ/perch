//! Namespacing: the one rule that keeps every server from colliding with a builtin.

mod support;

use perch_mcp::{McpHost, separate};
use perch_store::settings::McpServerConfig;
use perch_tools::ToolRegistry;
use support::fixture_server_named;

#[test]
fn a_registered_name_is_the_server_then_the_tool() {
    assert_eq!(separate("fixture", "echo"), "fixture__echo");
    assert_eq!(separate("my-server", "read"), "my-server__read");
}

/// The session will own the host beside its other state and call it from a worker thread, so
/// it has to be shareable. A runtime is not `Clone`, which is why this is worth pinning.
#[test]
fn the_host_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<McpHost>();
}

/// A remote server with no reachable endpoint must not abort startup: the host starts with
/// no tools from it and records why, so the rest of the app still runs.
#[test]
fn a_server_that_cannot_connect_is_reported_not_fatal() {
    let mut registry = ToolRegistry::builtin();
    let broken = McpServerConfig {
        name: "dead".to_string(),
        command: None,
        args: Vec::new(),
        url: Some("http://127.0.0.1:1/unreachable".to_string()),
    };
    let host =
        McpHost::connect_all(&[fixture_server_named("fixture"), broken], &mut registry).unwrap();

    assert_eq!(host.server_names(), vec!["fixture".to_string()]);
    assert!(
        host.note_lines()
            .iter()
            .any(|line| line.contains("dead") && line.contains("failed")),
        "the failure was not reported: {:?}",
        host.note_lines()
    );
    assert!(
        registry
            .specs()
            .iter()
            .any(|spec| spec.name == "fixture__echo")
    );
}
