//! The commands that change the window or a session's name, by their effect: an app pressed, and
//! what changed because of it. The file and terminal commands are in `effects_files`.

use perch_ui::Shell;
use perch_ui::shell::COMMANDS;
use tempfile::TempDir;

use crate::harness::{Fake, notes};

/// A shell with one session named `one` on a throwaway worktree.
pub fn one() -> (TempDir, Shell<Fake>) {
    let dir = tempfile::tempdir().unwrap();
    let fake = Fake::one("one", dir.path());
    let shell = Shell::new(fake.metas.clone(), 0).with_live(fake);
    (dir, shell)
}

/// Types `text` and presses Enter, as the composer does.
pub fn run(shell: &mut Shell<Fake>, text: &str) {
    shell.set_prompt(text);
    assert!(shell.submit(), "{text} is a command and is taken");
}

#[test]
fn help_writes_one_row_per_registered_command() {
    let (_dir, mut shell) = one();
    run(&mut shell, "/help");
    let rows = notes(&shell);
    assert_eq!(rows.len(), COMMANDS.len(), "{rows:?}");
    for command in COMMANDS {
        assert!(
            rows.iter()
                .any(|row| row.starts_with(&format!("/{}", command.name))),
            "{} is missing from {rows:?}",
            command.name
        );
    }
}

#[test]
fn rename_changes_the_stored_title_and_the_row_reads_it() {
    let (_dir, mut shell) = one();
    run(&mut shell, "/rename Renamed session");
    assert_eq!(
        shell.live().unwrap().renamed,
        Some((0, "Renamed session".to_string()))
    );
    assert_eq!(shell.tab_titles(), ["Renamed session".to_string()]);
}

#[test]
fn rename_without_a_title_says_what_is_missing() {
    let (_dir, mut shell) = one();
    run(&mut shell, "/rename");
    assert!(
        notes(&shell)
            .iter()
            .any(|note| note == "rename needs a title: /rename <title>")
    );
    assert_eq!(
        shell.tab_titles(),
        ["one".to_string()],
        "nothing was renamed"
    );
}

#[test]
fn theme_switches_the_kit_the_frame_is_drawn_with_and_saves_it() {
    let (_dir, mut shell) = one();
    assert!(shell.dark(), "the saved default is dark");
    run(&mut shell, "/theme light");
    assert!(!shell.dark(), "the frame now draws the light kit");
    assert_eq!(shell.live().unwrap().theme.as_deref(), Some("light"));
    run(&mut shell, "/theme");
    assert!(shell.dark(), "a bare /theme toggles back");
}

#[test]
fn themes_lists_the_machine_and_an_unknown_name_is_refused_in_words() {
    let (_dir, mut shell) = one();
    run(&mut shell, "/themes");
    let said = notes(&shell).join("\n");
    for builtin in ["dark", "light"] {
        assert!(
            said.contains(builtin),
            "the two built-ins are always listed, and {builtin} was not: {said}"
        );
    }
    assert!(
        !said.contains("not wired"),
        "a registered command is wired to something: {said}"
    );

    // A name the library does not have is refused here, in words that say what exists, and the
    // app is never asked to save a theme it cannot draw.
    run(&mut shell, "/theme sunset");
    let refused = notes(&shell).join("\n");
    assert!(
        refused.contains("sunset"),
        "the refusal names what was asked for: {refused}"
    );
    assert!(
        refused.contains("dark"),
        "and what exists, so the next try can succeed: {refused}"
    );
    assert_eq!(
        shell.live().unwrap().theme,
        None,
        "nothing was saved: {refused}"
    );
}

#[test]
fn model_sets_an_id_or_opens_the_list() {
    let (_dir, mut shell) = one();
    run(&mut shell, "/model some/id");
    assert_eq!(
        shell.live().unwrap().chosen_model.as_deref(),
        Some("some/id")
    );
    run(&mut shell, "/model");
    assert!(
        shell.settings_open(),
        "the list took the transcript's place"
    );
}

#[test]
fn sessions_takes_the_keyboard_and_quit_asks_the_window_to_close() {
    let (_dir, mut shell) = one();
    run(&mut shell, "/sessions");
    assert!(shell.sessions_focused());
    run(&mut shell, "/quit");
    assert!(shell.quit_requested());
    assert!(shell.take_quit(), "the close request is taken once");
    assert!(!shell.quit_requested());
}

#[test]
fn ordinary_text_is_still_a_prompt_and_an_unknown_slash_is_refused() {
    let (_dir, mut shell) = one();
    shell.set_prompt("hello there");
    assert!(shell.submit());
    assert_eq!(shell.live().unwrap().prompts, ["hello there".to_string()]);

    run(&mut shell, "/nope");
    assert!(
        notes(&shell)
            .iter()
            .any(|note| note.contains("/nope is not a command"))
    );
    assert_eq!(
        shell.live().unwrap().prompts.len(),
        1,
        "nothing else reached the model"
    );
}
