//! The tree as git reports it: ignored files, deletions, and the directories a deletion invents.

use perch_files::{FileStatus, NodeKind, Workspace};

use crate::support::{commit_all, flatten, temp_repo, walk};

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
