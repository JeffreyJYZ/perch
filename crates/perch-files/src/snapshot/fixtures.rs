//! A real repository for the snapshot tests, made and thrown away inside each test.

use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

/// Runs `git` in `dir` and answers its stdout, trimmed; a failure fails the test.
pub fn git(dir: &Path, args: &[&str]) -> String {
    let out = run(dir, args);
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Runs `git` in `dir` and answers its stdout as the bytes it printed.
pub fn git_bytes(dir: &Path, args: &[&str]) -> Vec<u8> {
    let out = run(dir, args);
    assert!(out.status.success(), "git {args:?} failed");
    out.stdout
}

/// Identity and signing forced off, so no global configuration decides a test's outcome.
fn run(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
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
        .output()
        .unwrap()
}

/// A repository with two committed files.
pub fn repo() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "--quiet"]);
    std::fs::write(dir.path().join("a.txt"), "one\n").unwrap();
    std::fs::write(dir.path().join("b.txt"), "two\n").unwrap();
    git(dir.path(), &["add", "--all"]);
    git(dir.path(), &["commit", "--quiet", "-m", "fixture"]);
    dir
}

/// The index file's bytes, which a capture must leave exactly as they were.
pub fn index_bytes(dir: &Path) -> Vec<u8> {
    let git_dir = git(dir, &["rev-parse", "--absolute-git-dir"]);
    std::fs::read(Path::new(&git_dir).join("index")).unwrap()
}
