//! `/undo` as the column says it: one line per path the app restored, then the conversation line.

use perch_ui::Shell;
use tempfile::TempDir;

use crate::harness::{Fake, notes};

/// A shell over a session on a throwaway worktree, with `undo` as the app's answer.
fn one(undo: Result<Vec<String>, String>) -> (TempDir, Shell<Fake>) {
    let dir = tempfile::tempdir().unwrap();
    let fake = Fake {
        undo: Some(undo),
        ..Fake::one("one", dir.path())
    };
    let shell = Shell::new(fake.metas.clone(), 0).with_live(fake);
    (dir, shell)
}

/// Types `text` and presses Enter, as the composer does.
fn run(shell: &mut Shell<Fake>, text: &str) {
    shell.set_prompt(text);
    assert!(shell.submit(), "{text} is a command and is taken");
}

#[test]
fn every_restored_path_is_said_and_the_conversation_is_stated_to_be_kept() {
    let (_dir, mut shell) = one(Ok(vec![
        "restored a.txt".to_string(),
        "restored b.txt".to_string(),
        "removed new.txt".to_string(),
    ]));

    run(&mut shell, "/undo");

    let said = notes(&shell);
    assert_eq!(
        &said[..3],
        ["restored a.txt", "restored b.txt", "removed new.txt"],
        "one line per path, in the order the app put them back: {said:?}"
    );
    assert!(
        said.iter()
            .any(|note| note.contains("the conversation is unchanged")),
        "and the half a restore cannot undo is said, not left to be guessed: {said:?}"
    );
    assert_eq!(said.len(), 4, "nothing else is said: {said:?}");
}

#[test]
fn a_refusal_is_said_as_the_app_wrote_it() {
    let (_dir, mut shell) = one(Err(
        "a turn is running, so nothing was restored: stop it first".to_string(),
    ));

    run(&mut shell, "/undo");

    let said = notes(&shell);
    assert_eq!(
        said,
        ["a turn is running, so nothing was restored: stop it first"],
        "the refusal is one sentence, exactly as the app gave it: {said:?}"
    );
}

#[test]
fn a_window_with_no_app_says_so_rather_than_pretending() {
    let mut shell = Shell::<Fake>::new(Vec::new(), 0);

    run(&mut shell, "/undo");

    let said = notes(&shell);
    assert!(
        said.iter()
            .any(|note| note == "no app is running, so nothing was restored"),
        "{said:?}"
    );
}
