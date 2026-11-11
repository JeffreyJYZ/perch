//! What the registry holds: the built-ins, a registered tool, approvals, and removal.

use std::sync::Arc;

use perch_tools::{Error, ToolCtx, ToolRegistry};
use serde_json::json;
use tempfile::TempDir;

use crate::echo::Echo;

#[test]
fn every_builtin_tool_is_registered_with_a_schema() {
    let specs = ToolRegistry::builtin().specs();
    let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "read",
            "write",
            "edit",
            "bash",
            "jobs",
            "list",
            "grep",
            "glob",
            "patch",
            "todowrite",
            "todoread"
        ]
    );

    for spec in &specs {
        assert!(
            !spec.description.is_empty(),
            "{} has no description",
            spec.name
        );
        let schema = &spec.parameters;
        assert_eq!(
            schema["type"], "object",
            "{} must declare an object schema, got {schema}",
            spec.name
        );
        assert!(
            schema["properties"].is_object(),
            "{} must declare properties, got {schema}",
            spec.name
        );
    }
}

#[test]
fn a_registered_tool_appears_in_the_specs_and_is_callable() {
    let mut registry = ToolRegistry::builtin();
    registry.register(Arc::new(Echo));

    assert!(registry.specs().iter().any(|s| s.name == "echo"));

    let ctx = ToolCtx {
        root: std::env::temp_dir(),
    };
    assert_eq!(
        registry
            .call("echo", &json!({"text": "hi"}), &ctx)
            .unwrap()
            .text,
        "hi"
    );
    assert!(matches!(
        registry.call("missing", &json!({}), &ctx),
        Err(Error::UnknownTool(name)) if name == "missing"
    ));
}

#[test]
fn approvals_are_required_only_for_mutating_tools() {
    let registry = ToolRegistry::builtin();
    for name in ["read", "list", "grep", "glob", "jobs"] {
        assert!(
            !registry.needs_approval(name).unwrap(),
            "{name} needs approval"
        );
    }
    for name in ["write", "edit", "bash", "patch"] {
        assert!(
            registry.needs_approval(name).unwrap(),
            "{name} needs no approval"
        );
    }
    let mut registry = registry;
    registry.register(Arc::new(Echo));
    assert!(!registry.needs_approval("echo").unwrap());
    assert!(registry.needs_approval("missing").is_err());
}

#[test]
fn unregistering_removes_the_spec_and_the_call_path() {
    let mut registry = ToolRegistry::builtin();
    registry.register(Arc::new(Echo));
    assert!(registry.unregister("echo"));
    assert!(!registry.specs().iter().any(|s| s.name == "echo"));
    let ctx = ToolCtx {
        root: TempDir::new().unwrap().path().to_path_buf(),
    };
    assert!(registry.call("echo", &json!({"text": "hi"}), &ctx).is_err());
    assert!(
        !registry.unregister("echo"),
        "second removal reported a change"
    );
}
