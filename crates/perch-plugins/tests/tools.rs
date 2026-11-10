//! Integration tests for the wasm plugin host: registration, trap isolation and the fact that
//! the ABI offers a plugin no way to read a file.

use std::path::{Path, PathBuf};

use perch_plugins::PluginHost;
use perch_tools::{ToolCtx, ToolRegistry};
use serde_json::json;

/// The checked-in fixtures live beside the crate, and a test's working directory is not
/// something to rely on.
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn ctx() -> ToolCtx {
    ToolCtx {
        root: tempfile::tempdir().unwrap().path().to_path_buf(),
    }
}

#[test]
fn a_plugin_tool_registers_and_runs() {
    let mut registry = ToolRegistry::builtin();
    let host = PluginHost::load(&fixtures(), &mut registry).unwrap();

    assert_eq!(host.len(), 1);
    assert!(registry.specs().iter().any(|s| s.name == "echo"));

    let out = registry
        .call("echo", &json!({"text": "hi"}), &ctx())
        .unwrap();
    assert_eq!(out.text, "hi");
}

#[test]
fn a_trapping_plugin_is_disabled_and_its_tools_vanish() {
    let mut registry = ToolRegistry::builtin();
    let host = PluginHost::load(&fixtures(), &mut registry).unwrap();

    assert!(registry.call("explode", &json!({}), &ctx()).is_err());
    assert!(!host.names().contains(&"explode".to_string()));
    assert!(!registry.specs().iter().any(|s| s.name == "explode"));
}

#[test]
fn a_plugin_cannot_read_outside_the_session_root() {
    let mut registry = ToolRegistry::builtin();
    PluginHost::load(&fixtures(), &mut registry).unwrap();

    let out = registry.call("read_outside", &json!({"path": "/etc/hosts"}), &ctx());
    assert!(
        out.is_err(),
        "a plugin read {out:?} with no host capability"
    );
}
