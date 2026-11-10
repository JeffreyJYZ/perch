//! Listing: entries inside the root only, and the same confinement on read.

use perch_tools::ToolRegistry;
use serde_json::json;

use crate::support::root_of;

#[test]
fn list_shows_entries_inside_the_root_only() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("sub")).unwrap();
    std::fs::write(root.path().join("a.txt"), "x").unwrap();
    let ctx = root_of(&root);
    let registry = ToolRegistry::builtin();

    let out = registry.call("list", &json!({"path": "."}), &ctx).unwrap();
    assert!(out.text.contains("a.txt"), "output was {}", out.text);
    assert!(out.text.contains("sub/"), "output was {}", out.text);

    assert!(
        registry
            .call("list", &json!({"path": "../"}), &ctx)
            .is_err()
    );
    assert!(
        registry
            .call("read", &json!({"path": "/etc/hosts"}), &ctx)
            .is_err()
    );
}
