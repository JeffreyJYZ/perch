//! Glob: a known file found by pattern, ignored files skipped, an escaping pattern refused.

use perch_tools::ToolRegistry;
use serde_json::json;

use crate::support::root_of;

#[test]
fn glob_finds_a_known_file_honours_gitignore_and_refuses_an_escape() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("src/deep")).unwrap();
    std::fs::write(root.path().join("src/deep/known.rs"), "fn main() {}\n").unwrap();
    std::fs::write(root.path().join("src/other.txt"), "x\n").unwrap();
    std::fs::write(root.path().join("building.rs"), "fn x() {}\n").unwrap();
    std::fs::write(root.path().join(".gitignore"), "building.rs\n").unwrap();
    let ctx = root_of(&root);
    let registry = ToolRegistry::builtin();

    let out = registry
        .call("glob", &json!({"pattern": "**/*.rs"}), &ctx)
        .unwrap();
    assert_eq!(
        out.text.trim(),
        "src/deep/known.rs",
        "output was {}",
        out.text
    );
    assert!(
        !out.text.contains("other.txt"),
        "the pattern was not applied: {}",
        out.text
    );

    let escaped = registry.call("glob", &json!({"pattern": "../**/*.rs"}), &ctx);
    assert!(
        escaped.is_err(),
        "a pattern that leaves the worktree must be refused, got {escaped:?}"
    );
    let absolute = registry.call("glob", &json!({"pattern": "/etc/**/*.conf"}), &ctx);
    assert!(absolute.is_err(), "an absolute pattern must be refused");
}
