//! Confinement: a path may not climb out of the root, by `..` or by symlink.

use std::path::Path;

use perch_tools::confine;

#[test]
fn parent_traversal_and_symlink_escape_are_refused() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("ok.txt"), "x").unwrap();

    assert!(confine(root.path(), Path::new("../etc/passwd")).is_err());
    assert!(confine(root.path(), Path::new("ok.txt")).is_ok());
    assert!(confine(root.path(), Path::new("nested/new.txt")).is_ok());

    std::os::unix::fs::symlink("/etc", root.path().join("out")).unwrap();
    assert!(confine(root.path(), Path::new("out/hosts")).is_err());
}
