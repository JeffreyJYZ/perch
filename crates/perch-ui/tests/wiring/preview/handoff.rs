//! What a press on a row does: fills the preview, and hands the path to the app's editor.

use std::sync::Arc;

use perch_files::Workspace;
use perch_ui::{COLUMN, Shell};

use crate::harness::{Fake, click, commit_all, find, meta, temp_repo_with, window, words};

use super::probe::{bounds, selection};

#[test]
fn a_row_press_fills_the_preview_and_its_button_hands_the_path_to_the_editor() {
    let repo = temp_repo_with(&[("a.rs", "fn a() {}\nfn b() {}\n")]);
    let workspace = Workspace::open(repo.path()).expect("the worktree opens");
    let editor = Arc::new(Fake::default());
    let shell = Shell::with_workspace(vec![meta("work")], 0, workspace).with_editor(editor.clone());
    let mut runtime = window(shell);

    // The row names the file, and a press on it is what selects it.
    click(&mut runtime, "file-row-0");
    assert_eq!(
        selection(&runtime),
        Some(repo.path().canonicalize().unwrap().join("a.rs")),
        "the row's press selected the file it names: {}",
        words(&runtime)
    );

    // The preview is in the centre column, with the file's lines and the one action.
    let shown = words(&runtime);
    assert!(shown.contains("fn a() {}"), "the preview draws the file");
    assert!(shown.contains("Open in Zed"), "the action is offered");
    let [x, _, width, _] = bounds(&runtime, "preview");
    assert!(
        x >= COLUMN,
        "the preview starts right of the sidebar: {x} vs {COLUMN}"
    );
    assert!(
        width > 700.0,
        "the preview takes the centre column's width, not the sidebar's: {width}"
    );

    click(&mut runtime, "open-in-zed");
    assert_eq!(
        editor.opened(),
        [repo.path().canonicalize().unwrap().join("a.rs")],
        "the button handed the selected path to the app's editor, and nothing else"
    );
}

#[test]
fn the_button_does_nothing_until_a_file_is_selected() {
    let repo = temp_repo_with(&[("a.rs", "fn a() {}\n")]);
    let workspace = Workspace::open(repo.path()).expect("the worktree opens");
    let editor = Arc::new(Fake::default());
    let runtime =
        window(Shell::with_workspace(vec![meta("work")], 0, workspace).with_editor(editor.clone()));
    let tree = runtime.accessibility_json();
    assert!(
        find(&tree, "preview").is_none(),
        "with nothing selected the centre column is the transcript: {tree}"
    );
    assert!(
        find(&tree, "open-in-zed").is_none(),
        "there is no action to press into nothing: {tree}"
    );
    assert!(editor.opened().is_empty(), "nothing was handed over");
}

#[test]
fn a_deleted_row_says_in_words_why_it_cannot_be_previewed() {
    let repo = temp_repo_with(&[("gone.rs", "x")]);
    commit_all(&repo);
    std::fs::remove_file(repo.path().join("gone.rs")).unwrap();
    let workspace = Workspace::open(repo.path()).expect("the worktree opens");
    let editor = Arc::new(Fake::default());
    let mut runtime =
        window(Shell::with_workspace(vec![meta("work")], 0, workspace).with_editor(editor.clone()));

    // The row is still there, and the tree already says what git reports.
    assert!(
        words(&runtime).contains("gone.rs, deleted"),
        "a tracked file git reports deleted keeps its row: {}",
        words(&runtime)
    );

    click(&mut runtime, "file-row-0");
    let tree = runtime.accessibility_json();
    let preview = find(&tree, "preview").expect("the reason is drawn where a preview would be");
    assert!(
        preview.to_string().contains("deleted"),
        "the panel says the deletion in words instead of an empty page: {preview}"
    );
    assert!(
        editor.opened().is_empty(),
        "nothing that is not there is handed to the editor"
    );
}

#[test]
fn a_refused_handoff_is_said_in_words_beside_the_preview() {
    let repo = temp_repo_with(&[("a.rs", "fn a() {}\n")]);
    let workspace = Workspace::open(repo.path()).expect("the worktree opens");
    let editor = Arc::new(Fake {
        refusal: Some("Zed is not installed".to_string()),
        ..Fake::default()
    });
    let mut runtime =
        window(Shell::with_workspace(vec![meta("work")], 0, workspace).with_editor(editor));

    click(&mut runtime, "file-row-0");
    click(&mut runtime, "open-in-zed");
    let shown = words(&runtime);
    assert!(
        shown.contains("Zed did not open") && shown.contains("Zed is not installed"),
        "the refusal is drawn where the file is: {shown}"
    );
    assert!(
        shown.contains("fn a() {}"),
        "a refusal keeps the preview on screen: {shown}"
    );
}
