//! Patch: several files in one call, an all-or-nothing failure, and the preview.

use perch_tools::{Preview, ToolRegistry};
use serde_json::json;

use crate::support::root_of;

const TWO_FILES: &str = "\
--- a/a.txt
+++ b/a.txt
@@ -1,3 +1,3 @@
 alpha
-beta
+beta changed
 gamma
--- a/b.txt
+++ b/b.txt
@@ -1,2 +1,2 @@
 one
-two
+two changed
";

/// The first file is correct and the second hunk's context does not exist.
const BAD_SECOND_FILE: &str = "\
--- a/a.txt
+++ b/a.txt
@@ -1,3 +1,3 @@
 alpha
-beta
+beta changed
 gamma
--- a/b.txt
+++ b/b.txt
@@ -1,2 +1,2 @@
 one
-TWO
+two changed
";

fn root_with_two_files() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("a.txt"), "alpha\nbeta\ngamma\n").unwrap();
    std::fs::write(root.path().join("b.txt"), "one\ntwo\n").unwrap();
    root
}

fn read(root: &tempfile::TempDir, name: &str) -> String {
    std::fs::read_to_string(root.path().join(name)).unwrap()
}

#[test]
fn a_two_file_diff_applies_every_file_and_reports_the_summary() {
    let root = root_with_two_files();
    let ctx = root_of(&root);

    let out = ToolRegistry::builtin()
        .call("patch", &json!({"diff": TWO_FILES}), &ctx)
        .unwrap();
    assert!(out.text.contains("2 files"), "summary was {}", out.text);
    assert!(out.text.contains("+2"), "summary was {}", out.text);
    assert!(matches!(out.preview, Some(Preview::Diff(_))));

    assert_eq!(read(&root, "a.txt"), "alpha\nbeta changed\ngamma\n");
    assert_eq!(read(&root, "b.txt"), "one\ntwo changed\n");
}

#[test]
fn a_hunk_that_does_not_match_writes_nothing_at_all() {
    let root = root_with_two_files();
    let ctx = root_of(&root);

    let error = ToolRegistry::builtin()
        .call("patch", &json!({"diff": BAD_SECOND_FILE}), &ctx)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("b.txt") && error.contains("hunk 1"),
        "the error must name the file and the hunk: {error}"
    );

    assert_eq!(
        read(&root, "a.txt"),
        "alpha\nbeta\ngamma\n",
        "the first file was written even though the second failed"
    );
    assert_eq!(read(&root, "b.txt"), "one\ntwo\n");
}

#[test]
fn a_patch_preview_shows_the_diff_and_writes_nothing() {
    let root = root_with_two_files();
    let ctx = root_of(&root);

    let preview = ToolRegistry::builtin()
        .preview("patch", &json!({"diff": TWO_FILES}), &ctx)
        .unwrap();
    assert!(
        matches!(preview, Some(Preview::Diff(ref diff)) if diff.contains("+beta changed")),
        "preview was {preview:?}"
    );
    assert_eq!(read(&root, "a.txt"), "alpha\nbeta\ngamma\n");
    assert_eq!(read(&root, "b.txt"), "one\ntwo\n");
}

#[test]
fn a_diff_against_dev_null_creates_the_file() {
    let root = tempfile::tempdir().unwrap();
    let ctx = root_of(&root);
    let diff = "\
--- /dev/null
+++ b/new.txt
@@ -0,0 +1,2 @@
+hello
+world
";

    let out = ToolRegistry::builtin()
        .call("patch", &json!({"diff": diff}), &ctx)
        .unwrap();
    assert!(out.text.contains("1 file"), "summary was {}", out.text);
    assert_eq!(read(&root, "new.txt"), "hello\nworld\n");
}
