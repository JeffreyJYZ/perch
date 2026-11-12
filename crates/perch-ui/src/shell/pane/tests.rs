//! The pane as state: a file opens a tab, reopening it focuses that tab, and closing the tab the
//! pane is showing returns to the transcript with the tree's selection untouched.

use std::path::PathBuf;

use perch_files::Workspace;

use super::{Pane, Shell};

/// A window over a throwaway worktree holding `files`.
fn shell_with(files: &[(&str, &str)]) -> (tempfile::TempDir, Shell) {
    let repo = tempfile::tempdir().unwrap();
    for (path, contents) in files {
        let path = repo.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }
    let workspace = Workspace::open(repo.path()).unwrap();
    (repo, Shell::with_workspace(Vec::new(), 0, workspace))
}

/// The root the shell resolved, which is the panel's own: absolute, and canonical where a
/// temporary directory is reached through a symlink.
fn root(shell: &Shell) -> PathBuf {
    shell
        .workspace()
        .expect("the window has a worktree")
        .root()
        .to_path_buf()
}

#[test]
fn a_file_opens_a_tab_and_reopening_it_focuses_the_same_one() {
    let (_repo, mut shell) = shell_with(&[("a.rs", "a"), ("b.rs", "b")]);
    let a = root(&shell).join("a.rs");
    assert!(shell.open_file("a.rs".as_ref()), "a live file previews");
    assert_eq!(shell.pane(), &Pane::File(a.clone()));
    assert_eq!(shell.file_tabs(), std::slice::from_ref(&a));

    assert!(shell.open_file("b.rs".as_ref()));
    assert_eq!(
        shell.file_tabs(),
        &[a.clone(), root(&shell).join("b.rs")],
        "a second file takes the next tab, in the order it was opened"
    );

    assert!(shell.open_file("a.rs".as_ref()));
    assert_eq!(
        shell.file_tabs().len(),
        2,
        "reopening a file focuses its tab and adds no second"
    );
    assert_eq!(shell.pane(), &Pane::File(a), "the focused tab is the pane");
}

#[test]
fn closing_the_shown_tab_returns_to_the_session_and_keeps_the_tree_selection() {
    let (_repo, mut shell) = shell_with(&[("a.rs", "a")]);
    shell.open_file("a.rs".as_ref());
    shell.close_file("a.rs".as_ref());

    assert_eq!(shell.pane(), &Pane::Session);
    assert!(shell.file_tabs().is_empty(), "the tab is gone");
    assert!(
        shell
            .files()
            .and_then(|panel| panel.selected_path())
            .is_some(),
        "the tree keeps the selection, so the file is one press away"
    );
}

#[test]
fn leaving_a_file_pane_keeps_its_tab_and_the_session_is_not_a_pane_to_leave() {
    let (_repo, mut shell) = shell_with(&[("a.rs", "a")]);
    shell.open_file("a.rs".as_ref());
    assert!(shell.leave_pane(), "a file pane is a pane to leave");
    assert_eq!(shell.pane(), &Pane::Session);
    assert_eq!(shell.file_tabs().len(), 1, "leaving the pane keeps the tab");
    assert!(!shell.leave_pane(), "the session is not a pane to leave");
}

#[test]
fn two_files_of_one_name_in_different_folders_get_a_tab_each() {
    let (_repo, mut shell) = shell_with(&[("one/a.rs", "a"), ("two/a.rs", "b")]);
    shell.open_file("one/a.rs".as_ref());
    shell.open_file("two/a.rs".as_ref());
    assert_eq!(shell.file_tabs().len(), 2);
    assert_eq!(shell.pane(), &Pane::File(root(&shell).join("two/a.rs")));
}
