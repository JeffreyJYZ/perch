//! The card's keys: the focused Enter, the window's chord, and the Escape that shuts it.

use perch_agent::AgentEvent;
use perch_agent::ToolCall;
use rune::{Key, KeyEvent, Mods};

use super::{card_escape, toggles_card};
use crate::shell::{Idle, Intent, Shell, Waiting, intent};

fn press(key: Key, mods: Mods) -> KeyEvent {
    KeyEvent { key, mods }
}

fn call(id: &str) -> AgentEvent {
    AgentEvent::ToolCall(ToolCall {
        id: id.to_string(),
        name: "read".to_string(),
        args: serde_json::json!({ "path": "src/main.rs" }),
    })
}

/// A shell holding one card per output, each finished.
fn shell_with(outputs: &[&str]) -> Shell<Idle> {
    let mut shell = Shell::idle(Vec::new(), 0);
    for (index, output) in outputs.iter().enumerate() {
        let id = format!("call-{index}");
        shell.transcript_mut().apply(&call(&id));
        shell.transcript_mut().apply(&AgentEvent::ToolResult {
            id,
            ok: true,
            text: (*output).to_string(),
        });
    }
    shell
}

#[test]
fn enter_and_space_open_a_card_and_a_chord_never_does() {
    assert!(toggles_card(&press(Key::Enter, Mods::default())));
    assert!(toggles_card(&press(
        Key::Text(" ".to_string()),
        Mods::default()
    )));
    for mods in [
        Mods {
            cmd: true,
            ..Mods::default()
        },
        Mods {
            ctrl: true,
            ..Mods::default()
        },
        Mods {
            alt: true,
            ..Mods::default()
        },
    ] {
        assert!(
            !toggles_card(&press(Key::Enter, mods)),
            "a chord belongs to a shortcut, not to the card: {mods:?}"
        );
    }
    assert!(!toggles_card(&press(
        Key::Text("x".to_string()),
        Mods::default()
    )));
}

#[test]
fn the_card_the_key_names_is_the_one_toggled() {
    let mut shell = shell_with(&["one", "two"]);

    assert!(shell.toggle_card("tool-0"), "the first card is a card");
    assert!(shell.transcript().is_expanded("call-0"));

    assert!(shell.toggle_card("tool-0"), "a second press closes it");
    assert!(!shell.transcript().is_expanded("call-0"));

    assert!(
        shell.toggle_card("tool-0-head"),
        "the header a press focused names the same card"
    );
    assert!(
        shell.transcript().is_expanded("call-0"),
        "and opens it: a header press must not strand the keyboard"
    );

    assert!(
        !shell.toggle_card("tool-4"),
        "a key past the last card names nothing"
    );
    assert!(
        !shell.toggle_card("note-0"),
        "only a tool card has something to open"
    );
    assert!(!shell.toggle_card("tool-x"), "an index must be a number");
}

#[test]
fn a_card_with_no_output_stays_shut_and_says_why() {
    let mut shell = Shell::idle(Vec::new(), 0);
    shell.transcript_mut().apply(&call("call-0"));

    assert!(
        !shell.toggle_card("tool-0"),
        "a call that has not answered has nothing to open"
    );
    assert!(!shell.transcript().is_expanded("call-0"));
}

#[test]
fn escape_shuts_the_opened_card_and_nothing_else_claims_it() {
    let mut shell = shell_with(&["one"]);
    let escape = press(Key::Escape, Mods::default());

    assert!(
        !card_escape(&mut shell, &escape),
        "Escape with nothing open is not this window's press"
    );
    shell.toggle_card("tool-0");
    assert!(card_escape(&mut shell, &escape), "Escape shuts it");
    assert!(!shell.transcript().is_expanded("call-0"));
    assert!(
        !card_escape(&mut shell, &escape),
        "and a second Escape has nothing left to shut"
    );

    shell.toggle_card("tool-0");
    let chorded = press(
        Key::Escape,
        Mods {
            cmd: true,
            ..Mods::default()
        },
    );
    assert!(
        !card_escape(&mut shell, &chorded),
        "a chord belongs to the shortcut that owns it"
    );
    assert!(shell.transcript().is_expanded("call-0"));
}

#[test]
fn the_chord_opens_the_focused_card_else_the_newest_one() {
    let mut shell = shell_with(&["one", "two"]);

    assert!(
        shell.open_card_at(None),
        "with nothing focused the newest opens"
    );
    assert!(shell.transcript().is_expanded("call-1"));

    assert!(
        shell.open_card_at(Some("tool-0-head")),
        "the focused card wins"
    );
    assert!(shell.transcript().is_expanded("call-0"));

    let mut empty = Shell::idle(Vec::new(), 0);
    assert!(
        !empty.open_card_at(None),
        "a column with no card has nothing to open"
    );
}

#[test]
fn command_o_is_the_chord_and_a_bare_o_is_a_character() {
    let open = press(
        Key::Command('o'),
        Mods {
            cmd: true,
            ..Mods::default()
        },
    );
    for waiting in [
        Waiting::Nothing,
        Waiting::Approval,
        Waiting::Question {
            options: 2,
            multiple: false,
        },
    ] {
        assert_eq!(intent(&open, waiting), Intent::Open, "{waiting:?}");
    }
    assert_eq!(
        intent(
            &press(Key::Text("o".to_string()), Mods::default()),
            Waiting::Nothing
        ),
        Intent::Ignore
    );
}
