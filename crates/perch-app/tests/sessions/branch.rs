//! The branch a session row carries: read when the session opens, re-read when it is shown, and
//! said in words — `HEAD` when detached, empty when the directory is not a repository.

use std::path::Path;
use std::process::Command;

use perch_app::AppState;

use crate::harness::{defaults, fixture, laid_out, sidebar};

/// Runs git in `dir` and asserts it worked, with the machine's own git config out of the way.
fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?} in {}", dir.display());
}

/// A repository on `main` with one commit pushed to a bare origin, then one commit ahead of it.
fn ahead_repo(root: &Path) -> std::path::PathBuf {
    let remote = root.join("remote.git");
    Command::new("git")
        .arg("init")
        .arg("--bare")
        .arg(&remote)
        .status()
        .unwrap();
    let local = root.join("local");
    std::fs::create_dir_all(&local).unwrap();
    git(&local, &["init", "-b", "main"]);
    std::fs::write(local.join("one.txt"), "one\n").unwrap();
    git(&local, &["add", "."]);
    git(&local, &["commit", "-m", "one"]);
    git(
        &local,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&local, &["push", "-u", "origin", "main"]);
    std::fs::write(local.join("two.txt"), "two\n").unwrap();
    git(&local, &["add", "."]);
    git(&local, &["commit", "-m", "two"]);
    local
}

#[test]
fn a_repo_one_commit_ahead_names_its_branch_and_its_drift() {
    let (data, roots, _one, _two) = fixture();
    let local = ahead_repo(roots.path());
    let mut app = AppState::open_with(data.path(), defaults()).unwrap();
    app.new_session(&local).unwrap();
    assert_eq!(app.metas()[0].branch, "main ↑1");

    let runtime = laid_out(app);
    let words = sidebar(&runtime);
    assert!(words.contains("main ↑1"), "{words}");
}

#[test]
fn a_plain_directory_has_no_branch_and_the_row_still_renders() {
    let (data, roots, _one, two) = fixture();
    let mut app = AppState::open_with(data.path(), defaults()).unwrap();
    app.new_session(&two).unwrap();
    assert_eq!(app.metas()[0].branch, "");
    let runtime = laid_out(app);
    let words = sidebar(&runtime);
    assert!(words.contains("no branch"), "{words}");
    assert!(words.contains("two"), "the row is drawn: {words}");
}

#[test]
fn a_detached_head_is_the_word_head() {
    let (data, roots, _one, _two) = fixture();
    let local = ahead_repo(roots.path());
    git(&local, &["checkout", "--detach", "HEAD"]);
    let mut app = AppState::open_with(data.path(), defaults()).unwrap();
    app.new_session(&local).unwrap();
    assert_eq!(app.metas()[0].branch, "HEAD");
}

#[test]
fn a_checkout_is_reflected_when_the_session_is_shown_again() {
    let (data, roots, _one, _two) = fixture();
    let local = ahead_repo(roots.path());
    let mut app = AppState::open_with(data.path(), defaults()).unwrap();
    app.new_session(&local).unwrap();
    assert_eq!(app.metas()[0].branch, "main ↑1");

    git(&local, &["checkout", "-b", "feature"]);
    app.switch(0).unwrap();
    assert_eq!(
        app.metas()[0].branch,
        "feature",
        "the row follows the worktree, not the open"
    );
}
