//! `/diff` when there is nothing to show, or when the file named is not one of the turn's.

use serde_json::json;

use crate::diff::{changed_twice, path, run, shell_on};
use crate::harness::notes;

#[test]
fn a_file_the_turn_did_not_touch_has_no_diff_to_show() {
    let dir = changed_twice();
    let mut shell = shell_on(dir.path(), &[("edit", path("a.txt"))]);

    run(&mut shell, "/diff c.txt");

    let said = notes(&shell);
    assert!(
        said.iter().any(|note| note
            == "diff: c.txt was not changed by the last turn, so there is no diff of it here"),
        "{said:?}"
    );
}

#[test]
fn a_path_outside_the_worktree_is_refused_and_never_read() {
    let dir = changed_twice();
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("secret.txt");
    std::fs::write(&secret, "PRIVATE\n").unwrap();
    let mut shell = shell_on(dir.path(), &[("write", json!({ "path": secret }))]);

    run(&mut shell, "/diff");

    let said = notes(&shell);
    assert!(
        said.iter()
            .any(|note| note.contains("outside the worktree")),
        "the refusal names the reason: {said:?}"
    );
    assert!(
        !said.iter().any(|note| note.contains("PRIVATE")),
        "and the file was never read: {said:?}"
    );
}

#[test]
fn a_directory_that_is_not_a_repository_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut shell = shell_on(dir.path(), &[("write", path("a.txt"))]);

    run(&mut shell, "/diff");

    let said = notes(&shell);
    assert!(
        said.iter()
            .any(|note| note.contains("is not a git worktree")),
        "{said:?}"
    );
}

#[test]
fn a_transcript_with_no_finished_turn_is_refused() {
    let dir = changed_twice();
    let mut shell = shell_on(dir.path(), &[]);

    run(&mut shell, "/diff");

    let said = notes(&shell);
    assert!(
        said.iter()
            .any(|note| note.contains("no finished turn is in the transcript")),
        "{said:?}"
    );
}

#[test]
fn a_worktree_nobody_changed_refuses_instead_of_printing_an_empty_box() {
    let dir = changed_twice();
    // The turn names a file it did not actually change: git sees nothing, so there is no diff.
    let mut shell = shell_on(dir.path(), &[("write", path("c.txt"))]);

    run(&mut shell, "/diff");

    let said = notes(&shell);
    assert!(
        said.iter()
            .any(|note| note == "diff: the last turn changed no file"),
        "{said:?}"
    );
}
