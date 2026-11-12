//! The autocomplete list, typed into a real composer: `/` opens it, letters narrow it, Escape
//! closes it and keeps the text, and Enter takes a row before the next Enter runs it.

use perch_ui::Shell;
use rune::{Key, KeyEvent, Mods, Runtime};

use crate::harness::{Fake, window};

/// Clicks the composer's field so the keys land in it, as a person's first click does.
fn focus_composer(runtime: &mut Runtime<Shell<Fake>>) {
    let field = runtime
        .elements()
        .into_iter()
        .find(|element| element.key.as_deref() == Some("composer-field"))
        .expect("the composer's field is in the tree");
    let at = (
        field.rect[0] + field.rect[2] * 0.5,
        field.rect[1] + field.rect[3] * 0.5,
    );
    runtime.pointer_down(at.0, at.1);
    runtime.pointer_up(at.0, at.1);
    runtime.frame(0.0);
}

/// Types `text` one press per character.
fn type_text(runtime: &mut Runtime<Shell<Fake>>, text: &str) {
    for ch in text.chars() {
        runtime.key(KeyEvent {
            key: Key::Text(ch.to_string()),
            mods: Mods::default(),
        });
        runtime.frame(0.0);
    }
}

fn press(runtime: &mut Runtime<Shell<Fake>>, key: Key) {
    runtime.key(KeyEvent {
        key,
        mods: Mods::default(),
    });
    runtime.frame(0.0);
}

#[test]
fn a_slash_opens_the_list_and_letters_narrow_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut runtime = window(Fake::one("one", dir.path()));
    focus_composer(&mut runtime);

    type_text(&mut runtime, "/");
    let rows = runtime
        .state
        .command_menu()
        .expect("the list is open above the composer");
    assert_eq!(rows.len(), perch_ui::shell::COMMANDS.len());
    assert_eq!(runtime.state.prompt_text(), "/");

    type_text(&mut runtime, "ren");
    let rows = runtime.state.command_menu().unwrap();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].name, "rename");
}

#[test]
fn escape_closes_the_list_and_keeps_what_was_typed() {
    let dir = tempfile::tempdir().unwrap();
    let mut runtime = window(Fake::one("one", dir.path()));
    focus_composer(&mut runtime);
    type_text(&mut runtime, "/ren");
    assert!(runtime.state.command_menu().is_some());

    press(&mut runtime, Key::Escape);
    assert!(runtime.state.command_menu().is_none(), "the list closed");
    assert_eq!(runtime.state.prompt_text(), "/ren", "the text stayed");
}

#[test]
fn enter_takes_a_row_and_the_next_enter_runs_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut runtime = window(Fake::one("one", dir.path()));
    focus_composer(&mut runtime);
    type_text(&mut runtime, "/comp");
    press(&mut runtime, Key::Enter);
    assert_eq!(
        runtime.state.prompt_text(),
        "/compact",
        "Enter completes the row instead of sending it"
    );
    assert!(runtime.state.command_menu().is_none());

    press(&mut runtime, Key::Enter);
    assert!(
        runtime.state.live().unwrap().compacted,
        "the second Enter ran the command"
    );
}

#[test]
fn tab_takes_the_first_row_as_the_typed_form() {
    let dir = tempfile::tempdir().unwrap();
    let mut runtime = window(Fake::one("one", dir.path()));
    focus_composer(&mut runtime);
    type_text(&mut runtime, "/");
    press(&mut runtime, Key::Tab);
    assert_eq!(runtime.state.prompt_text(), "/new");
    assert!(runtime.state.command_menu().is_none());
}
