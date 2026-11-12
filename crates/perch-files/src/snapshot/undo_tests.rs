//! What `/undo` does with the snapshot: the files back, and one refusal per reason it cannot.

use super::Undo;
use super::fixtures::{git, repo};

#[test]
fn undo_puts_the_bytes_back_and_removes_what_the_turn_created() {
    let repo = repo();
    let root = repo.path();
    let mut undo = Undo::default();
    undo.capture(root);

    std::fs::write(root.join("a.txt"), "one\nchanged\n").unwrap();
    std::fs::write(root.join("new.txt"), "new\n").unwrap();
    std::fs::remove_file(root.join("b.txt")).unwrap();

    let lines = undo
        .restore(root, false)
        .expect("the turn's files come back");

    assert_eq!(
        lines,
        ["restored a.txt", "restored b.txt", "removed new.txt"],
        "every path is named as it is put back"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("a.txt")).unwrap(),
        "one\n",
        "the old bytes are back"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("b.txt")).unwrap(),
        "two\n",
        "the deleted file is back"
    );
    assert!(!root.join("new.txt").exists(), "the created file is gone");
}

#[test]
fn an_executable_bit_comes_back_with_the_bytes() {
    use std::os::unix::fs::PermissionsExt;
    let repo = repo();
    let root = repo.path();
    std::fs::write(root.join("run.sh"), "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(root.join("run.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    git(root, &["add", "--all"]);
    git(root, &["commit", "--quiet", "-m", "script"]);

    let mut undo = Undo::default();
    undo.capture(root);
    std::fs::set_permissions(root.join("run.sh"), std::fs::Permissions::from_mode(0o644)).unwrap();
    std::fs::write(root.join("run.sh"), "#!/bin/sh\necho hi\n").unwrap();

    undo.restore(root, false).unwrap();

    let mode = std::fs::metadata(root.join("run.sh"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o755, "a script comes back runnable");
}

#[test]
fn a_directory_that_is_not_a_repository_has_no_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let mut undo = Undo::default();
    undo.capture(dir.path());

    let words = undo.restore(dir.path(), false).unwrap_err();

    assert!(words.contains("is not a git worktree"), "{words}");
}

#[test]
fn a_turn_that_is_running_is_never_restored_under() {
    let repo = repo();
    let mut undo = Undo::default();
    undo.capture(repo.path());

    let words = undo.restore(repo.path(), true).unwrap_err();

    assert!(words.contains("a turn is running"), "{words}");
}

#[test]
fn a_turn_that_changed_nothing_has_nothing_to_restore() {
    let repo = repo();
    let mut undo = Undo::default();
    undo.capture(repo.path());

    let words = undo.restore(repo.path(), false).unwrap_err();

    assert!(words.contains("nothing has changed"), "{words}");
}

#[test]
fn undoing_twice_says_the_files_were_already_put_back() {
    let repo = repo();
    let root = repo.path();
    let mut undo = Undo::default();
    undo.capture(root);
    std::fs::write(root.join("a.txt"), "changed\n").unwrap();
    undo.restore(root, false).expect("the first undo restores");

    let words = undo.restore(root, false).unwrap_err();

    assert!(words.contains("already put back"), "{words}");
}

#[test]
fn a_new_turn_starts_a_new_snapshot() {
    let repo = repo();
    let root = repo.path();
    let mut undo = Undo::default();
    undo.capture(root);
    std::fs::write(root.join("a.txt"), "changed\n").unwrap();
    undo.restore(root, false).unwrap();

    // The next turn's start is what `/undo` restores to, not the one before it.
    undo.capture(root);
    std::fs::write(root.join("a.txt"), "changed again\n").unwrap();
    let lines = undo.restore(root, false).expect("the new turn is undone");

    assert_eq!(
        lines,
        ["restored a.txt"],
        "the second turn's own boundary is the one put back"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("a.txt")).unwrap(),
        "one\n",
        "which is what the worktree held when it began"
    );
}

#[test]
fn no_turn_has_started_is_said_rather_than_pretended() {
    let repo = repo();
    let mut undo = Undo::default();

    let words = undo.restore(repo.path(), false).unwrap_err();

    assert!(words.contains("no turn has started"), "{words}");
}
