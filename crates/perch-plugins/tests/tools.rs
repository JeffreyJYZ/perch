//! Integration tests for the wasm plugin host: registration, trap isolation and the fact that
//! the ABI offers a plugin no way to read a file.

use std::path::{Path, PathBuf};

use perch_plugins::PluginHost;
use perch_tools::{Error, ToolCtx, ToolRegistry};
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

    // Nothing is disabled yet, so releasing removes nothing.
    assert_eq!(host.release(&mut registry), 0);

    // Every tool the fixture declares is a real registry entry under its own name.
    for name in ["echo", "explode", "read_outside"] {
        let specs = registry.specs();
        let spec = specs
            .iter()
            .find(|spec| spec.name == name)
            .unwrap_or_else(|| panic!("{name} was never registered"));
        assert!(!spec.name.is_empty());
    }

    assert!(registry.call("explode", &json!({}), &ctx()).is_err());
    assert!(!host.names().contains(&"explode".to_string()));
    assert_eq!(host.len(), 0);

    // A trap disables the plugin but does not touch the registry, so the tools are still there
    // under their real names and refuse as disabled rather than vanishing silently.
    let err = registry.call("explode", &json!({}), &ctx()).unwrap_err();
    assert!(
        matches!(err, Error::Message(ref message) if message.contains("disabled")),
        "a disabled plugin answered {err:?} instead of a disabled error"
    );
    assert!(registry.specs().iter().any(|spec| spec.name == "explode"));

    // Removal is the registry's, run by whoever owns `&mut ToolRegistry`.
    assert_eq!(host.release(&mut registry), 3);
    for name in ["echo", "explode", "read_outside"] {
        assert!(
            !registry.specs().iter().any(|spec| spec.name == name),
            "{name} is still offered after release"
        );
    }
    assert!(
        !registry.specs().iter().any(|spec| spec.name.is_empty()),
        "a spec with no name survived"
    );

    // Gone by name, so dispatch answers unknown-tool rather than a plugin error.
    let err = registry.call("explode", &json!({}), &ctx()).unwrap_err();
    assert!(
        matches!(err, Error::UnknownTool(ref name) if name == "explode"),
        "a released tool answered {err:?} instead of an unknown-tool error"
    );

    // The built-ins were never part of the removal.
    assert!(registry.specs().iter().any(|spec| spec.name == "bash"));
    assert_eq!(host.release(&mut registry), 0);
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
