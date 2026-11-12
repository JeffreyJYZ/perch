//! What a capture is: a tree object the worktree's own content, and nothing of the person's state
//! disturbed.

use super::fixtures::{git, git_bytes, index_bytes, repo};
use super::{capture, changes};

#[test]
fn a_capture_leaves_the_index_head_the_stash_and_the_worktree_alone() {
    let repo = repo();
    let root = repo.path();
    std::fs::write(root.join("a.txt"), "one\nchanged\n").unwrap();
    std::fs::write(root.join("new.txt"), "new\n").unwrap();
    std::fs::write(root.join("b.txt"), "two\nstaged\n").unwrap();
    git(root, &["add", "b.txt"]);

    let status = git(root, &["status", "--porcelain"]);
    let index = index_bytes(root);
    let head = git(root, &["rev-parse", "HEAD"]);
    let stash = git(root, &["stash", "list"]);

    let tree = capture(root).expect("the worktree is a repository");

    assert_eq!(
        git(root, &["cat-file", "-t", &tree]),
        "tree",
        "a tree object, not a commit"
    );
    assert_eq!(
        git(root, &["status", "--porcelain"]),
        status,
        "git status is byte for byte what it was"
    );
    assert_eq!(index_bytes(root), index, "the person's index is untouched");
    assert_eq!(git(root, &["rev-parse", "HEAD"]), head, "no ref moved");
    assert_eq!(git(root, &["stash", "list"]), stash, "no stash was made");
    assert_eq!(
        std::fs::read_to_string(root.join("a.txt")).unwrap(),
        "one\nchanged\n",
        "the worktree is what it was"
    );
    assert_eq!(
        git(root, &["rev-list", "--all", "--count"]),
        "1",
        "no commit was written"
    );
}

#[test]
fn a_capture_holds_the_worktree_as_it_is() {
    let repo = repo();
    let root = repo.path();
    std::fs::write(root.join("a.txt"), "one\nchanged\n").unwrap();
    std::fs::write(root.join("new.txt"), "new\n").unwrap();

    let tree = capture(root).unwrap();

    assert_eq!(
        git_bytes(root, &["cat-file", "blob", &format!("{tree}:a.txt")]),
        b"one\nchanged\n",
        "the tree holds the file as the worktree has it"
    );
    assert_eq!(
        git_bytes(root, &["cat-file", "blob", &format!("{tree}:new.txt")]),
        b"new\n",
        "and a file nobody has committed yet"
    );
}

#[test]
fn the_changes_are_what_the_worktree_holds_now_against_the_tree() {
    let repo = repo();
    let root = repo.path();
    let tree = capture(root).unwrap();
    std::fs::write(root.join("a.txt"), "one\nchanged\n").unwrap();
    std::fs::write(root.join("new.txt"), "new\n").unwrap();
    std::fs::remove_file(root.join("b.txt")).unwrap();

    let listed = changes(root, &tree).unwrap();
    let mut paths: Vec<&str> = listed.iter().map(|change| change.path.as_str()).collect();
    paths.sort_unstable();
    assert_eq!(paths, ["a.txt", "b.txt", "new.txt"]);

    let mode = |path: &str| {
        listed
            .iter()
            .find(|change| change.path == path)
            .and_then(|change| change.mode)
    };
    assert!(
        mode("a.txt").is_some(),
        "a file the snapshot has keeps its mode"
    );
    assert!(mode("b.txt").is_some(), "so does one the turn deleted");
    assert_eq!(mode("new.txt"), None, "one the turn created has none");
}
