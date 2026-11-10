//! Integration tests for the worktree browser.

use std::path::Path;
use std::process::Command;

use perch_files::{FileStatus, Node, Workspace, language_for};
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
