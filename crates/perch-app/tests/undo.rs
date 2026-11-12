//! `/undo` as the app answers it: which refusal a session in each state gives.
//!
//! The worktree is a repository this test makes and throws away; no turn is ever run, so nothing
//! reaches a provider and nothing reads the machine the suite runs in.

mod support;

use std::process::Command;

use perch_app::{AppState, ProviderDefaults};
use perch_store::secret::MemorySecrets;
use perch_ui::shell::Live;

use support::hermetic;

/// A repository with one committed file, made and thrown away by the test that asks for it.
fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "one\n").unwrap();
    for args in [
        vec!["init", "--quiet"],
        vec!["add", "--all"],
        vec!["commit", "--quiet", "-m", "fixture"],
    ] {
        let status = Command::new("git")
            .args([
                "-c",
                "user.email=perch-test@example.com",
                "-c",
                "user.name=perch test",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(&args)
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(status.status.success(), "git {args:?} failed");
    }
    dir
}

/// An app over a throwaway data directory, with no key and no provider request.
fn app(data: &tempfile::TempDir) -> AppState {
    hermetic();
    AppState::open_with_secrets(
        data.path(),
        ProviderDefaults::default(),
        Box::new(MemorySecrets::default()),
    )
    .expect("a data directory with no sessions opens")
}

#[test]
fn a_session_whose_turn_never_started_says_there_is_nothing_to_restore() {
    let data = tempfile::tempdir().unwrap();
    let worktree = repo();
    let mut app = app(&data);
    app.new_session(worktree.path()).expect("a session opens");

    let words = Live::undo(&mut app).expect_err("no turn has started");

    assert!(words.contains("no turn has started"), "{words}");
}

#[test]
fn a_window_with_no_session_says_so() {
    let data = tempfile::tempdir().unwrap();
    let mut app = app(&data);

    let words = app.undo_turn().expect_err("no session is open");

    assert!(words.contains("no session is open"), "{words}");
}

#[test]
fn a_worktree_that_is_not_a_repository_cannot_be_snapshotted() {
    let data = tempfile::tempdir().unwrap();
    let worktree = tempfile::tempdir().unwrap();
    let mut app = app(&data);
    app.new_session(worktree.path()).expect("a session opens");

    let words = app
        .undo_turn()
        .expect_err("a plain directory has no snapshot");

    assert!(words.contains("is not a git worktree"), "{words}");
}
