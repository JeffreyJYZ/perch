//! A saved allow covers the call it was written for and nothing beside it.
//!
//! This is the case that matters most: a rule that quietly grew into "everything" turns one
//! approval into a standing permission for the whole disk, and nothing in the UI would say so.

use std::path::Path;

use serde_json::json;

use perch_agent::dispatch::Decision;
use perch_agent::{Permission, Rules};

use crate::harness::at;

#[test]
fn a_saved_allow_does_not_widen_to_another_tool_or_escape_the_worktree() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/main.rs"), "fn main() {}").unwrap();
    let mut rules = at(tmp.path());
    rules
        .remember("write", &json!({ "path": "src/" }), Decision::Allow)
        .unwrap();

    let root = tmp.path();
    let allow = Some(Permission::Allow);
    assert_eq!(
        rules.permit("write", &json!({ "path": "src/main.rs" }), root),
        allow
    );
    assert_eq!(
        rules.permit("write", &json!({ "path": "src/deep/new.rs" }), root),
        allow
    );

    refuse(&rules, "write", "/etc/passwd", root, "an absolute escape");
    refuse(&rules, "write", "../outside.rs", root, "a relative escape");
    refuse(&rules, "edit", "src/main.rs", root, "another tool");
    refuse(&rules, "write", "srcx/main.rs", root, "a prefixed sibling");
}

/// Asserts that nothing saved covers this call, because a rule that widened is the defect.
fn refuse(rules: &Rules, tool: &str, arg: &str, root: &Path, why: &str) {
    assert_eq!(
        rules.permit(tool, &json!({ "path": arg }), root),
        None,
        "{why}: {arg}"
    );
}

#[cfg(unix)]
#[test]
fn a_symlink_out_of_the_worktree_is_not_covered_by_a_saved_allow() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::create_dir_all(tmp.path().join("outside")).unwrap();
    std::fs::write(tmp.path().join("outside/secret"), "x").unwrap();
    std::os::unix::fs::symlink(tmp.path().join("outside"), tmp.path().join("src/link")).unwrap();

    let mut rules = at(tmp.path());
    rules
        .remember("write", &json!({ "path": "src/" }), Decision::Allow)
        .unwrap();

    assert_eq!(
        rules.permit("write", &json!({ "path": "src/link/secret" }), tmp.path()),
        None,
        "confinement resolves the link before the rule is applied"
    );
}
