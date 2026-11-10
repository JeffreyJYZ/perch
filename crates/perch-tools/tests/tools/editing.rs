//! Editing: a diff preview, a refusal outside the root, and old text that must be unique.

use perch_tools::{Preview, ToolRegistry};
use serde_json::json;

use crate::support::root_of;

#[test]
fn edit_returns_a_diff_preview_and_write_refuses_outside_root() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("a.rs"), "fn a() {}\n").unwrap();
    let ctx = root_of(&root);

    let out = ToolRegistry::builtin()
        .call(
            "edit",
            &json!({"path": "a.rs", "old": "a", "new": "b"}),
            &ctx,
        )
        .unwrap();
    assert!(matches!(out.preview, Some(Preview::Diff(_))));
    assert_eq!(
        std::fs::read_to_string(root.path().join("a.rs")).unwrap(),
        "fn b() {}\n"
    );

    assert!(
        ToolRegistry::builtin()
            .call("write", &json!({"path": "../x", "text": "y"}), &ctx)
            .is_err()
    );
}

#[test]
fn edit_refuses_old_text_that_is_absent_or_ambiguous() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("a.rs"), "let a = 1;\nlet a = 2;\n").unwrap();
    let ctx = root_of(&root);
    let registry = ToolRegistry::builtin();

    let ambiguous = registry.call(
        "edit",
        &json!({"path": "a.rs", "old": "let a", "new": "let b"}),
        &ctx,
    );
    assert!(ambiguous.is_err(), "old text twice must be refused");
    let absent = registry.call(
        "edit",
        &json!({"path": "a.rs", "old": "nowhere", "new": "x"}),
        &ctx,
    );
    assert!(absent.is_err(), "absent old text must be refused");
    assert_eq!(
        std::fs::read_to_string(root.path().join("a.rs")).unwrap(),
        "let a = 1;\nlet a = 2;\n",
        "a refused edit must not touch the file"
    );
}
