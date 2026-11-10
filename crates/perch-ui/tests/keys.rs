//! The window's key map: what one press asks for, given what a session is waiting on.
//!
//! These are the presses themselves, with no app and no layout: a press in, an intent out. The
//! window that carries each intent out is `wiring::answer` for a question and `window` in
//! perch-app for a whole turn.

use perch_ui::shell::{Intent, Typing, Waiting, intent, typing_intent};
use rune::{Key, KeyEvent, Mods};

fn press(key: Key, shift: bool) -> KeyEvent {
    KeyEvent {
        key,
        mods: Mods {
            shift,
            ..Mods::default()
        },
    }
}

fn chord(key: Key) -> KeyEvent {
    KeyEvent {
        key,
        mods: Mods {
            cmd: true,
            ..Mods::default()
        },
    }
}

fn text(value: &str) -> KeyEvent {
    KeyEvent {
        key: Key::Text(value.to_string()),
        mods: Mods::default(),
    }
}

const SINGLE: Waiting = Waiting::Question {
    options: 3,
    multiple: false,
};
const MULTI: Waiting = Waiting::Question {
    options: 3,
    multiple: true,
};
const FREE: Waiting = Waiting::Question {
    options: 0,
    multiple: false,
};

#[test]
fn enter_sends_and_shift_enter_breaks_the_line() {
    assert_eq!(
        intent(&press(Key::Enter, false), Waiting::Nothing),
        Intent::Submit
    );
    assert_eq!(
        intent(&press(Key::Enter, true), Waiting::Nothing),
        Intent::Newline
    );
    // An approval is a different Enter: it allows the call rather than sending the composer.
    assert_eq!(
        intent(&press(Key::Enter, false), Waiting::Approval),
        Intent::Approve
    );
}

#[test]
fn a_number_key_picks_and_enter_confirms() {
    for waiting in [SINGLE, MULTI] {
        assert_eq!(
            intent(&text("2"), waiting),
            Intent::Select(1),
            "{waiting:?}"
        );
        assert_eq!(
            intent(&text("9"), waiting),
            Intent::Ignore,
            "past the last option: {waiting:?}"
        );
        assert_eq!(
            intent(&text("0"), waiting),
            Intent::Ignore,
            "options count from one: {waiting:?}"
        );
        assert_eq!(intent(&press(Key::Enter, false), waiting), Intent::Confirm);
    }
    // With nothing waiting, a digit is typing: the field keeps it.
    assert_eq!(intent(&text("2"), Waiting::Nothing), Intent::Ignore);
}

#[test]
fn t_opens_the_answer_field_and_a_question_with_no_options_opens_it_on_enter() {
    for waiting in [SINGLE, MULTI] {
        assert_eq!(intent(&text("t"), waiting), Intent::Type, "{waiting:?}");
        assert_eq!(intent(&text("T"), waiting), Intent::Type, "{waiting:?}");
    }
    // A question that offers nothing can only be answered in words, so Enter opens the field.
    assert_eq!(intent(&press(Key::Enter, false), FREE), Intent::Type);
    // Nothing else types while a question waits: an option is picked with a number or `t`.
    assert_eq!(intent(&text("yes"), SINGLE), Intent::Ignore);
    assert_eq!(intent(&text("t"), Waiting::Nothing), Intent::Ignore);
}

#[test]
fn command_enter_allows_for_the_session_while_an_approval_waits() {
    assert_eq!(
        intent(&chord(Key::Enter), Waiting::Approval),
        Intent::AllowForSession
    );
    for waiting in [Waiting::Nothing, SINGLE, MULTI] {
        assert_eq!(
            intent(&chord(Key::Enter), waiting),
            Intent::Ignore,
            "only an approval can be allowed for the session: {waiting:?}"
        );
    }
    // The chord is still not the field's: Shift is part of another shortcut, never this one.
    let shifted = KeyEvent {
        key: Key::Enter,
        mods: Mods {
            cmd: true,
            shift: true,
            ..Mods::default()
        },
    };
    assert_eq!(intent(&shifted, Waiting::Approval), Intent::Ignore);
}

#[test]
fn escape_refuses_an_approval_and_dismisses_a_question() {
    assert_eq!(
        intent(&press(Key::Escape, false), Waiting::Approval),
        Intent::Deny
    );
    assert_eq!(intent(&press(Key::Escape, false), SINGLE), Intent::Cancel);
    assert_eq!(intent(&press(Key::Escape, false), MULTI), Intent::Cancel);
    assert_eq!(
        intent(&press(Key::Escape, false), Waiting::Nothing),
        Intent::Ignore
    );
}

#[test]
fn cmd_comma_asks_for_the_settings() {
    let settings = KeyEvent {
        key: Key::Command(','),
        mods: Mods {
            cmd: true,
            ..Mods::default()
        },
    };
    for waiting in [Waiting::Nothing, Waiting::Approval, SINGLE] {
        assert_eq!(intent(&settings, waiting), Intent::Settings, "{waiting:?}");
    }
    // Without ⌘ a comma is a character in the field, not a shortcut.
    assert_eq!(intent(&text(","), Waiting::Nothing), Intent::Ignore);
}

#[test]
fn ctrl_c_interrupts_whatever_is_on_screen() {
    let interrupt = KeyEvent {
        key: Key::Text("c".to_string()),
        mods: Mods {
            ctrl: true,
            ..Mods::default()
        },
    };
    for waiting in [Waiting::Nothing, Waiting::Approval, SINGLE] {
        assert_eq!(intent(&interrupt, waiting), Intent::Cancel, "{waiting:?}");
    }
    // ⌘C is a copy, not an interrupt.
    let copy = KeyEvent {
        key: Key::Command('c'),
        mods: Mods {
            cmd: true,
            ..Mods::default()
        },
    };
    assert_eq!(intent(&copy, Waiting::Nothing), Intent::Ignore);
}

#[test]
fn the_typing_map_sends_on_enter_and_goes_back_on_escape() {
    assert_eq!(typing_intent(&press(Key::Enter, false)), Some(Typing::Send));
    assert_eq!(
        typing_intent(&press(Key::Escape, false)),
        Some(Typing::Back)
    );
    // Every other press is the field's: a digit typed here is a character in the answer, and
    // Shift+Enter is its own line break.
    assert_eq!(typing_intent(&text("2")), None);
    assert_eq!(typing_intent(&text(" ")), None);
    assert_eq!(typing_intent(&press(Key::Enter, true)), None);
}
