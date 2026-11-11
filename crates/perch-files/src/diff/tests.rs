//! Diff fixtures: a real repository, made and thrown away inside each test.

use std::path::Path;
use std::process::Command;

use super::*;
use tempfile::TempDir;

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args([
            "-c",
            "user.email=perch-test@example.com",
            "-c",
            "user.name=perch test",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed in {}", dir.display());
}

fn repo() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "--quiet"]);
    dir
}

fn commit_all(dir: &Path) {
    git(dir, &["add", "--all"]);
    git(dir, &["commit", "--quiet", "-m", "fixture"]);
}

#[test]
fn a_modified_file_diff_is_a_real_hunk() {
    let repo = repo();
    let file = repo.path().join("one.rs");
    std::fs::write(&file, "fn one() {}\n").unwrap();
    commit_all(repo.path());
    std::fs::write(&file, "fn two() {}\n").unwrap();

    let diff = file_diff(repo.path(), Path::new("one.rs")).expect("a modified file has a diff");
    assert!(diff.contains("@@"), "no hunk header: {diff}");
    assert!(diff.contains("-fn one() {}"), "no removed line: {diff}");
    assert!(diff.contains("+fn two() {}"), "no added line: {diff}");
}

#[test]
fn an_untracked_file_renders_as_added_lines() {
    let repo = repo();
    std::fs::write(repo.path().join("new.rs"), "fn new() {}\n").unwrap();

    let diff = file_diff(repo.path(), Path::new("new.rs")).expect("a new file has a diff");
    assert!(diff.contains("--- /dev/null"), "no new-file header: {diff}");
    assert!(diff.contains("+fn new() {}"), "no added line: {diff}");
}

#[test]
fn a_binary_file_is_named_not_copied() {
    let repo = repo();
    std::fs::write(repo.path().join("blob.bin"), [0u8, 159, 146, 150, 0]).unwrap();

    let diff = file_diff(repo.path(), Path::new("blob.bin")).expect("a binary file still diffs");
    assert!(diff.contains("Binary file blob.bin"), "not named: {diff}");
}

#[test]
fn a_path_outside_the_root_is_refused() {
    let repo = repo();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.txt"), "no\n").unwrap();

    assert_eq!(
        file_diff(repo.path(), &outside.path().join("secret.txt")),
        None
    );
    assert_eq!(file_diff(repo.path(), Path::new("/etc/hosts")), None);
    assert_eq!(file_diff(repo.path(), Path::new("../../etc/passwd")), None);

    std::os::unix::fs::symlink(outside.path(), repo.path().join("escape")).unwrap();
    assert_eq!(
        file_diff(repo.path(), Path::new("escape/secret.txt")),
        None,
        "a symlink out of the root is an escape"
    );
}

#[test]
fn a_huge_diff_is_cut_and_says_so() {
    let repo = repo();
    let line = format!("{}\n", "x".repeat(1023));
    std::fs::write(repo.path().join("big.txt"), line.repeat(400)).unwrap();

    let diff = file_diff(repo.path(), Path::new("big.txt")).expect("a big file still diffs");
    assert!(diff.len() > DIFF_MAX_BYTES, "the cap did not apply");
    assert!(diff.contains("[diff cut: "), "the cut is not named");
    assert!(diff.contains("bytes shown]"), "the total is not named");
}

#[test]
fn the_worktree_diff_carries_tracked_and_untracked_changes() {
    let repo = repo();
    std::fs::write(repo.path().join("kept.rs"), "fn kept() {}\n").unwrap();
    commit_all(repo.path());
    std::fs::write(repo.path().join("kept.rs"), "fn changed() {}\n").unwrap();
    std::fs::write(repo.path().join("fresh.rs"), "fn fresh() {}\n").unwrap();

    let diff = worktree_diff(repo.path()).expect("a repository has a worktree diff");
    assert!(diff.contains("+fn changed() {}"), "tracked change missing");
    assert!(diff.contains("+fn fresh() {}"), "untracked file missing");
}

#[test]
fn a_plain_directory_is_no_diff() {
    let plain = tempfile::tempdir().unwrap();
    assert_eq!(worktree_diff(plain.path()), None);
    assert_eq!(file_diff(plain.path(), Path::new("nothing.rs")), None);
}
