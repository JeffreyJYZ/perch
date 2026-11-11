//! The commands that touch a file, a terminal, the data directory or a new session — each by
//! its effect, and each refusal naming the one thing that is missing.

use perch_files::Workspace;
use perch_ui::Shell;

use crate::harness::{Fake, editor, notes};

use super::effects::{one, run};

#[test]
fn export_writes_a_file_under_the_data_directory() {
    let (worktree, mut shell) = one();
    let data = tempfile::tempdir().unwrap();
    shell.live_mut().unwrap().data = Some(data.path().to_path_buf());
    shell.transcript_mut().push_user("what is this?");
    run(&mut shell, "/export");

    let written = std::fs::read_dir(data.path())
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|extension| extension == "txt"))
        .expect("an export file");
    let text = std::fs::read_to_string(&written).unwrap();
    assert_eq!(text.lines().next(), Some("you: what is this?"));
    let said = notes(&shell).join(" ");
    assert!(said.contains("exported:"), "{said}");
    assert!(said.contains(&data.path().display().to_string()));
    drop(worktree);
}

#[test]
fn export_without_a_data_directory_refuses_in_words() {
    let (_dir, mut shell) = one();
    shell.transcript_mut().push_user("what is this?");
    run(&mut shell, "/export");
    assert!(
        notes(&shell)
            .iter()
            .any(|note| note.contains("no data directory"))
    );
}

#[test]
fn a_missing_precondition_names_itself_and_does_nothing_else() {
    let mut idle = Shell::idle(Vec::new(), 0);
    for (text, expected) in [
        ("/new", "no worktree is open, so no session was started"),
        ("/editor", "no file is open, so nothing was handed to Zed"),
        ("/compact", "no session is open, so nothing was compacted"),
        (
            "/sessions",
            "no session is open, so there is no list to focus",
        ),
        ("/clean", "no session is open, so there is nothing to clean"),
        (
            "/export",
            "no data directory is known, so nothing was exported",
        ),
        (
            "/rename a title",
            "no app is running, so the title was not saved",
        ),
    ] {
        let before = idle.transcript().items().len();
        idle.set_prompt(text);
        assert!(idle.submit());
        let said = idle.transcript().items()[before..]
            .iter()
            .filter_map(|item| match item {
                perch_ui::Item::Note(words) => Some(words.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ");
        assert!(said.contains(expected), "{text} said {said}");
        assert!(idle.terminals().is_empty(), "{text} opened no shell");
        assert_eq!(idle.session_count(), 0, "{text} made no session");
    }
}

#[test]
fn clean_empties_the_transcript_and_says_so() {
    let (_dir, mut shell) = one();
    shell.transcript_mut().push_user("old question");
    run(&mut shell, "/clean");
    let leftover = shell
        .transcript()
        .items()
        .iter()
        .any(|item| matches!(item, perch_ui::Item::User { .. }));
    assert!(!leftover, "the old turn is gone");
    assert!(notes(&shell).iter().any(|note| note.contains("cleaned")));
}

#[test]
fn new_opens_a_second_session_on_this_worktree() {
    let (worktree, mut shell) = one();
    run(&mut shell, "/new");
    assert_eq!(shell.session_count(), 2);
    let folder = worktree.path().file_name().unwrap().to_string_lossy();
    assert_eq!(shell.tab_titles()[1], folder, "the folder names the row");
}

#[test]
fn terminal_opens_a_tab_in_the_sessions_worktree() {
    let (_dir, mut shell) = one();
    run(&mut shell, "/terminal");
    assert_eq!(shell.terminals().len(), 1, "one tab was opened");
}

#[test]
fn editor_hands_the_open_file_to_zed() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("main.rs");
    std::fs::write(&file, "fn main() {}\n").unwrap();
    let workspace = Workspace::open(dir.path()).unwrap();
    let fake = Fake::one("one", dir.path());
    let recorder = editor();
    let mut shell = Shell::with_workspace(fake.metas.clone(), 0, workspace)
        .with_live(fake)
        .with_editor(recorder.clone());
    assert!(shell.open_file(&file));
    run(&mut shell, "/editor");
    assert_eq!(recorder.opened.lock().unwrap().as_slice(), [file]);
    assert!(
        notes(&shell)
            .iter()
            .any(|note| note.starts_with("opened in Zed"))
    );
}
