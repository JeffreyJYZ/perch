//! What a preview refuses: a deletion, and any path that leaves the root.

use std::path::Path;

use perch_files::Workspace;

use crate::support::{commit_all, temp_repo};

#[test]
fn previewing_a_deleted_file_reports_the_deletion() {
    let repo = temp_repo();
    std::fs::write(repo.path().join("gone.rs"), "fn gone() {}\n").unwrap();
    commit_all(&repo, "add gone.rs");
    std::fs::remove_file(repo.path().join("gone.rs")).unwrap();

    let ws = Workspace::open(repo.path()).unwrap();
    let error = ws
        .preview(Path::new("gone.rs"))
        .expect_err("a deleted file has nothing to preview");
    let message = error.to_string();
    assert!(
        message.contains("deleted in the worktree"),
        "message was: {message}"
    );
}

#[test]
fn preview_refuses_a_path_outside_the_root() {
    let repo = temp_repo();
    std::fs::write(repo.path().join("kept.rs"), "fn main() {}\n").unwrap();
    let ws = Workspace::open(repo.path()).unwrap();

    assert!(ws.preview(Path::new("../../etc/passwd")).is_err());

    std::os::unix::fs::symlink("/etc", repo.path().join("outside")).unwrap();
    assert!(
        ws.preview(Path::new("outside/hosts")).is_err(),
        "a symlink out of the root is an escape"
    );

    std::os::unix::fs::symlink("kept.rs", repo.path().join("alias.rs")).unwrap();
    let text = ws
        .preview(Path::new("alias.rs"))
        .expect("a symlink that stays inside the root is allowed");
    assert_eq!(text.lines, vec!["fn main() {}"]);
    assert_eq!(text.language.as_deref(), Some("rust"));
}
