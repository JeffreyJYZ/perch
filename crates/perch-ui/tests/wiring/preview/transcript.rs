//! Leaving the preview: the transcript comes back, keeps streaming, and the file stays selected.

use perch_agent::AgentEvent;
use perch_files::Workspace;
use perch_ui::Shell;

use crate::harness::{click, meta, temp_repo_with, window, words};

use super::probe::{escape, preview_shown, selection};

#[test]
fn escape_returns_the_transcript_and_keeps_the_file_selected() {
    let repo = temp_repo_with(&[("a.rs", "fn a() {}\n")]);
    let workspace = Workspace::open(repo.path()).expect("the worktree opens");
    let mut runtime = window(Shell::with_workspace(vec![meta("work")], 0, workspace));
    let selected = repo.path().canonicalize().unwrap().join("a.rs");

    click(&mut runtime, "file-row-0");
    assert_eq!(selection(&runtime), Some(selected.clone()));
    assert!(
        preview_shown(&runtime),
        "the preview replaced the transcript"
    );

    // The transcript keeps streaming while it is hidden: the shell is state, not a drawing.
    runtime
        .state
        .transcript_mut()
        .apply(&AgentEvent::TextDelta("still streaming".to_string()));
    runtime.frame(1.0 / 60.0);
    assert!(
        !words(&runtime).contains("still streaming"),
        "a hidden transcript is not drawn under the preview"
    );

    let shown = escape(&mut runtime);
    assert!(!preview_shown(&runtime), "Escape left the preview");
    assert!(
        shown.contains("still streaming"),
        "the transcript is back, including what streamed while it was hidden: {shown}"
    );
    assert_eq!(
        selection(&runtime),
        Some(selected),
        "leaving the preview does not clear the tree's selection"
    );
    assert!(
        shown.contains("a.rs") && shown.contains("selected"),
        "the tree still says the file is selected: {shown}"
    );
}

#[test]
fn the_header_button_returns_the_transcript_too() {
    let repo = temp_repo_with(&[("a.rs", "fn a() {}\n")]);
    let workspace = Workspace::open(repo.path()).expect("the worktree opens");
    let mut runtime = window(Shell::with_workspace(vec![meta("work")], 0, workspace));

    click(&mut runtime, "file-row-0");
    click(&mut runtime, "preview-close");
    assert!(
        !preview_shown(&runtime),
        "the header's button left the preview"
    );
    assert!(
        selection(&runtime).is_some(),
        "the file stays selected after the button"
    );
}
