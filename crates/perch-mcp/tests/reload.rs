//! Reconciling the connected set with the configured one: a kept server is not touched, a removed
//! one takes its tools with it, and a server that never connected is recorded and removable.

mod support;

use perch_mcp::McpHost;
use perch_store::settings::McpServerConfig;
use perch_tools::{ToolCtx, ToolRegistry};
use serde_json::json;
use support::fixture_server;

fn names(registry: &ToolRegistry) -> Vec<String> {
    registry.specs().into_iter().map(|spec| spec.name).collect()
}

#[test]
fn a_registry_built_from_empty_is_filled_without_reconnecting() {
    let mut registry = ToolRegistry::builtin();
    let host = McpHost::connect_all(&[fixture_server()], &mut registry).unwrap();
    let notes = host.notes().len();

    let mut rebuilt = ToolRegistry::builtin();
    host.register_into(&mut rebuilt);

    assert!(names(&rebuilt).contains(&"fixture__echo".to_string()));
    assert!(names(&rebuilt).contains(&"bash".to_string()));
    assert_eq!(
        host.notes().len(),
        notes,
        "filling a fresh registry reconnected the server"
    );

    let ctx = ToolCtx {
        root: tempfile::tempdir().unwrap().path().to_path_buf(),
    };
    assert_eq!(
        rebuilt
            .call("fixture__echo", &json!({"text": "hi"}), &ctx)
            .unwrap()
            .text,
        "hi",
        "the rebuilt registry's copy of the tool does not answer"
    );
}

#[test]
fn a_removed_server_takes_its_tools_with_it() {
    let mut registry = ToolRegistry::builtin();
    let host = McpHost::connect_all(&[fixture_server()], &mut registry).unwrap();

    let mut rebuilt = ToolRegistry::builtin();
    host.reconcile(&[], &mut rebuilt);

    assert!(host.server_names().is_empty(), "{:?}", host.server_names());
    assert!(
        !names(&rebuilt)
            .iter()
            .any(|name| name.starts_with("fixture__")),
        "the removed server's tools are still offered: {:?}",
        names(&rebuilt)
    );
    assert!(
        host.note_lines()
            .iter()
            .any(|line| line.contains("fixture") && line.contains("removed")),
        "the removal was not reported: {:?}",
        host.note_lines()
    );

    // Configured with nothing, it is not connected again on the next pass either.
    let mut third = ToolRegistry::builtin();
    host.reconcile(&[], &mut third);
    assert!(
        !names(&third)
            .iter()
            .any(|name| name.starts_with("fixture__"))
    );
}

#[test]
fn a_changed_server_is_reconnected_and_reregistered() {
    let mut registry = ToolRegistry::builtin();
    let host = McpHost::connect_all(&[fixture_server()], &mut registry).unwrap();

    // Same name, different arguments: the old connection cannot serve this configuration.
    let mut changed = fixture_server();
    changed.args = vec!["--renamed".to_string()];
    let mut rebuilt = ToolRegistry::builtin();
    host.reconcile(&[changed], &mut rebuilt);

    assert_eq!(host.server_names(), vec!["fixture".to_string()]);
    assert!(
        names(&rebuilt).contains(&"fixture__echo".to_string()),
        "the reconnected server contributed nothing: {:?}",
        names(&rebuilt)
    );
    assert!(
        host.note_lines()
            .iter()
            .filter(|line| line.contains("connected"))
            .count()
            >= 2,
        "the changed server was not reconnected: {:?}",
        host.note_lines()
    );
}

#[test]
fn a_server_that_never_connected_is_recorded_and_removable() {
    let broken = McpServerConfig::stdio("nowhere", "/nonexistent/perch-mcp-server", Vec::new());
    let mut registry = ToolRegistry::builtin();
    let host = McpHost::connect_all(&[broken], &mut registry).unwrap();

    assert!(host.server_names().is_empty());
    assert_eq!(
        host.failed().len(),
        1,
        "a server that never connected was not recorded: {:?}",
        host.failed()
    );
    assert_eq!(host.failed()[0].0, "nowhere");

    // Before it was recorded, `disconnect` had no entry to see and the server was invisible.
    assert_eq!(host.disconnect(&mut registry), 0);
    assert!(
        host.failed().is_empty(),
        "the dead server survived the disconnect: {:?}",
        host.failed()
    );
}
