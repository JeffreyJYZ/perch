//! Grep: binaries skipped, paths relative, regex patterns, gitignore and a missing `rg`.

use perch_tools::{Grep, Tool, ToolRegistry};
use serde_json::json;

use crate::support::root_of;

#[test]
fn grep_skips_binary_files_and_lists_matches_relative_to_the_root() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("text.txt"), "needle here\nnothing\n").unwrap();
    std::fs::write(root.path().join("bin.dat"), b"needle\0binary\n").unwrap();
    let ctx = root_of(&root);

    let out = ToolRegistry::builtin()
        .call("grep", &json!({"pattern": "needle", "path": "."}), &ctx)
        .unwrap();
    assert!(
        out.text.contains("text.txt:1:needle here"),
        "output was {}",
        out.text
    );
    assert!(
        !out.text.contains("./text.txt"),
        "paths must be relative to the root without a `./` prefix, output was {}",
        out.text
    );
    assert!(
        !out.text.contains("bin.dat"),
        "grep must skip binary files, output was {}",
        out.text
    );
}

#[test]
fn grep_treats_the_pattern_as_a_regex() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
    let ctx = root_of(&root);
    let registry = ToolRegistry::builtin();

    let out = registry
        .call("grep", &json!({"pattern": "fn\\s+main"}), &ctx)
        .unwrap();
    assert!(
        out.text.contains("main.rs:1:fn main() {}"),
        "a regex pattern must match whitespace runs, output was {}",
        out.text
    );

    let invalid = registry.call("grep", &json!({"pattern": "("}), &ctx);
    assert!(
        invalid.is_err(),
        "an invalid regex must be an error, got {invalid:?}"
    );
}

#[test]
fn grep_respects_gitignore() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("text.txt"), "needle here\n").unwrap();
    std::fs::write(root.path().join("ignored.txt"), "needle ignored\n").unwrap();
    std::fs::write(root.path().join(".gitignore"), "ignored.txt\n").unwrap();
    let ctx = root_of(&root);

    let out = ToolRegistry::builtin()
        .call("grep", &json!({"pattern": "needle", "path": "."}), &ctx)
        .unwrap();
    assert!(
        out.text.contains("text.txt:1:needle here"),
        "output was {}",
        out.text
    );
    assert!(
        !out.text.contains("ignored.txt"),
        "an ignored file must not be searched, output was {}",
        out.text
    );
}

#[test]
fn grep_reports_a_missing_rg_rather_than_matching_nothing() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("text.txt"), "needle here\n").unwrap();
    let ctx = root_of(&root);
    let grep = Grep::with_program("/nonexistent/perch-rg-probe");

    let err = grep
        .call(&json!({"pattern": "needle"}), &ctx)
        .expect_err("a missing rg must be an error, not an empty result");
    assert!(
        err.to_string().contains("perch-rg-probe"),
        "error must name the program, was {err}"
    );
}
