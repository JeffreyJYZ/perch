//! What the keys do to a live turn: Enter sends, Ctrl+C stops, Shift+Enter breaks the line.

use perch_ui::Item;
use perch_ui::Shell;
use perch_ui::shell::composer_key;
use rune::{Env, Key, KeyEvent, Mods, Runtime, Tx};

use crate::harness::{TIMEOUT, asked, press, pump_until, said, turns, window_against};

#[test]
fn enter_sends_the_composer_as_one_turn_and_the_app_owns_it() {
    let (mut shell, _data, _worktree) = window_against("tests/fixtures/live_text.txt");
    shell.set_prompt("what is this?");

    assert!(press(&mut shell, Key::Enter), "Enter is the send key");
    assert_eq!(
        shell.prompt_text(),
        "",
        "the box empties once the app took the text"
    );
    assert!(
        pump_until(&mut shell, |shell| turns(shell) == 1),
        "the turn should end within {TIMEOUT:?}"
    );

    assert_eq!(
        asked(&shell),
        ["what is this?".to_string()],
        "the app adds the user turn, and adds it once"
    );
    assert!(
        said(&shell)
            .iter()
            .any(|text| text.contains("Hello from the fixture")),
        "the reply streams into the column: {:?}",
        said(&shell)
    );

    let log = shell
        .live()
        .expect("the window kept its app")
        .active_session()
        .expect("the session is open")
        .log
        .events()
        .unwrap();
    assert_eq!(
        log.iter()
            .filter(|event| matches!(event, perch_store::SessionEvent::User { .. }))
            .count(),
        1,
        "the question is in the log exactly once: {log:?}"
    );
}

#[test]
fn ctrl_c_stops_the_running_turn() {
    let (mut shell, _data, _worktree) = window_against("tests/fixtures/live_slow.txt");
    shell.set_prompt("go");
    press(&mut shell, Key::Enter);

    assert!(
        pump_until(&mut shell, |shell| !said(shell).is_empty()),
        "the reply should start streaming within {TIMEOUT:?}"
    );

    let interrupt = KeyEvent {
        key: Key::Text("c".to_string()),
        mods: Mods {
            ctrl: true,
            ..Mods::default()
        },
    };
    assert!(
        composer_key(&mut shell, &interrupt, &mut Tx::default()),
        "Ctrl+C is consumed by the window, not typed"
    );
    assert!(
        pump_until(&mut shell, |shell| turns(shell) >= 1),
        "a cancel still ends the turn"
    );
    assert!(
        said(&shell)
            .iter()
            .any(|text| text.to_lowercase().contains("cancelled"))
            || shell.transcript().items().iter().any(
                |item| matches!(item, Item::Note(text) if text.to_lowercase().contains("cancelled"))
            ),
        "the column says the turn was cancelled: {:?}",
        said(&shell)
    );
}

#[test]
fn a_focused_composer_breaks_the_line_on_shift_enter() {
    let mut runtime = Runtime::new(
        Shell::idle(Vec::new(), 0),
        perch_ui::shell::view,
        (1280.0, 820.0),
        Env::new(true, 1.0),
    );
    runtime.frame(0.0);
    let field = runtime
        .elements()
        .into_iter()
        .find(|element| element.role == "textbox")
        .expect("the composer's field is in the tree");
    let at = (
        field.rect[0] + field.rect[2] * 0.5,
        field.rect[1] + field.rect[3] * 0.5,
    );
    runtime.pointer_down(at.0, at.1);
    runtime.pointer_up(at.0, at.1);
    runtime.frame(0.0);

    runtime.key(KeyEvent {
        key: Key::Text("a".to_string()),
        mods: Mods::default(),
    });
    runtime.frame(0.0);
    assert_eq!(runtime.state.prompt_text(), "a", "typing reaches the field");

    runtime.key(KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    });
    runtime.frame(0.0);
    assert_eq!(
        runtime.state.prompt_text(),
        "a",
        "Enter is the send key, never a character"
    );

    runtime.key(KeyEvent {
        key: Key::Enter,
        mods: Mods {
            shift: true,
            ..Mods::default()
        },
    });
    runtime.frame(0.0);
    assert_eq!(
        runtime.state.prompt_text(),
        "a\n",
        "Shift+Enter is the field's own line break"
    );
}
