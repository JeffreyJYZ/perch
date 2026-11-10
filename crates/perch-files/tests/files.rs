//! Integration tests for the worktree browser.

use std::path::Path;
use std::process::Command;

use perch_files::{FileStatus, Node, NodeKind, Workspace, language_for};
use tempfile::TempDir;

/// A throwaway repository, never the perch checkout.
fn temp_repo() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    assert!(
        status.success(),
        "git init failed in {}",
        dir.path().display()
    );
    dir
}

/// Stages everything and commits it, with identity and signing forced off so no global
/// configuration can change the outcome.
fn commit_all(repo: &TempDir, message: &str) {
    let staged = Command::new("git")
        .args(["add", "--all"])
        .current_dir(repo.path())
        .status()
        .unwrap();
    assert!(
        staged.success(),
        "git add failed in {}",
        repo.path().display()
    );
    let committed = Command::new("git")
        .args([
            "-c",
            "user.email=perch-test@example.com",
            "-c",
            "user.name=perch test",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            message,
        ])
        .current_dir(repo.path())
        .status()
        .unwrap();
    assert!(
        committed.success(),
        "git commit failed in {}",
        repo.path().display()
    );
}

fn walk(node: &Node) -> Vec<&Node> {
    let mut seen = vec![node];
    for child in &node.children {
        seen.extend(walk(child));
    }
    seen
}

fn flatten(nodes: &[Node]) -> Vec<String> {
    nodes
        .iter()
        .flat_map(walk)
        .map(|n| n.name.clone())
        .collect()
}

#[test]
fn tree_hides_ignored_files_and_marks_git_state() {
    let repo = temp_repo();
    std::fs::write(repo.path().join("kept.rs"), "fn main() {}").unwrap();
    std::fs::write(repo.path().join("ignored.log"), "x").unwrap();
    std::fs::write(repo.path().join(".gitignore"), "*.log\n").unwrap();
    let ws = Workspace::open(repo.path()).unwrap();
    let names = flatten(&ws.tree());
    assert!(names.contains(&"kept.rs".to_string()), "tree: {names:?}");
    assert!(
        !names.contains(&"ignored.log".to_string()),
        "tree: {names:?}"
    );
    let status = ws
        .tree()
        .iter()
        .flat_map(walk)
        .find(|n| n.name == "kept.rs")
        .unwrap()
        .status;
    assert_eq!(status, Some(FileStatus::Untracked));
}

#[test]
fn a_deleted_tracked_file_appears_greyed_under_its_folder() {
    let repo = temp_repo();
    std::fs::create_dir(repo.path().join("src")).unwrap();
    std::fs::write(repo.path().join("src/gone.rs"), "fn gone() {}\n").unwrap();
    commit_all(&repo, "add gone.rs");
    std::fs::remove_file(repo.path().join("src/gone.rs")).unwrap();

    let ws = Workspace::open(repo.path()).unwrap();
    let tree = ws.tree();
    let src = tree
        .iter()
        .find(|node| node.name == "src")
        .expect("src directory node");
    assert_eq!(src.kind, NodeKind::Dir);
    let gone = src
        .children
        .iter()
        .find(|node| node.name == "gone.rs")
        .expect("a deleted tracked file keeps its node");
    assert_eq!(gone.status, Some(FileStatus::Deleted));
    assert_eq!(gone.kind, NodeKind::File);
}

#[test]
fn deleting_a_whole_folder_creates_the_missing_parents() {
    let repo = temp_repo();
    std::fs::create_dir(repo.path().join("sub")).unwrap();
    std::fs::write(repo.path().join("sub/inner.rs"), "fn inner() {}\n").unwrap();
    commit_all(&repo, "add sub/inner.rs");
    std::fs::remove_dir_all(repo.path().join("sub")).unwrap();

    let ws = Workspace::open(repo.path()).unwrap();
    let tree = ws.tree();
    let sub = tree
        .iter()
        .find(|node| node.name == "sub")
        .expect("the deleted file's directory is invented on its path");
    assert_eq!(sub.kind, NodeKind::Dir);
    assert_eq!(
        sub.status, None,
        "an invented directory carries no status of its own"
    );
    let inner = sub
        .children
        .iter()
        .find(|node| node.name == "inner.rs")
        .expect("the deleted file sits under the invented directory");
    assert_eq!(inner.status, Some(FileStatus::Deleted));
    assert_eq!(
        tree.len(),
        1,
        "only the deleted path invents a directory: {tree:?}"
    );
}

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

#[test]
fn language_is_inferred_from_the_extension() {
    assert_eq!(language_for(Path::new("a.rs")), Some("rust"));
    assert_eq!(language_for(Path::new("CMakeLists.txt")), Some("cmake"));
    assert_eq!(language_for(Path::new("README")), None);
}
