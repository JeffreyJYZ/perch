//! Previews: a mutating tool states its change before it runs, and writes nothing.

use perch_tools::{Preview, ToolRegistry};
use serde_json::json;

use crate::support::root_of;

#[test]
fn mutating_tools_describe_the_change_before_it_runs() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("a.rs"), "fn a() {}\n").unwrap();
    let ctx = root_of(&root);
    let registry = ToolRegistry::builtin();

    assert_eq!(
        registry
            .preview("bash", &json!({"command": "ls"}), &ctx)
            .unwrap(),
        Some(Preview::Command("ls".into()))
    );

    let edit = registry
        .preview(
            "edit",
            &json!({"path": "a.rs", "old": "a", "new": "b"}),
            &ctx,
        )
        .unwrap();
    assert!(
        matches!(edit, Some(Preview::Diff(ref diff)) if diff.contains("-fn a() {}") && diff.contains("+fn b() {}")),
        "edit preview was {edit:?}"
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("a.rs")).unwrap(),
        "fn a() {}\n",
        "a preview must not write"
    );

    let write = registry
        .preview(
            "write",
            &json!({"path": "new.txt", "text": "hello\n"}),
            &ctx,
        )
        .unwrap();
    assert!(
        matches!(write, Some(Preview::Diff(ref diff)) if diff.contains("+hello")),
        "write preview was {write:?}"
    );
    assert!(
        !root.path().join("new.txt").exists(),
        "a preview must not create the file"
    );

    assert_eq!(
        registry
            .preview("read", &json!({"path": "a.rs"}), &ctx)
            .unwrap(),
        None,
        "a read-only tool has nothing to preview"
    );
    assert!(
        registry.preview("missing", &json!({}), &ctx).is_err(),
        "an unknown tool cannot be previewed"
    );
    assert!(
        registry
            .preview("write", &json!({"path": "../x", "text": "y"}), &ctx)
            .is_err(),
        "a preview must refuse a path outside the root"
    );
}
