//! A waiting call: Enter allows it, Command+Enter allows the tool for the session, Escape
//! refuses it.

use perch_ui::shell::Waiting;
use rune::{Key, Mods};

use crate::harness::{TIMEOUT, press, press_with, pump_until, turns, window_against};

/// The command chord, which is the second approval key.
fn command_enter() -> Mods {
    Mods {
        cmd: true,
        ..Mods::default()
    }
}

#[test]
fn enter_allows_the_call_that_is_waiting() {
    let (mut shell, _data, worktree) = window_against("tests/fixtures/live_write.txt");
    shell.set_prompt("write made.txt");
    press(&mut shell, Key::Enter);

    assert!(
        pump_until(&mut shell, |shell| shell.waiting() == Waiting::Approval),
        "the write should ask for approval within {TIMEOUT:?}"
    );
    assert!(
        !worktree.path().join("made.txt").exists(),
        "the call has not run while it waits"
    );

    assert!(
        press(&mut shell, Key::Enter),
        "Enter allows the waiting call"
    );
    assert!(
        pump_until(&mut shell, |shell| turns(shell) == 1),
        "the turn should finish once the call is allowed"
    );
    assert_eq!(
        std::fs::read_to_string(worktree.path().join("made.txt")).unwrap(),
        "hello from perch",
        "the allowed call ran"
    );
}

#[test]
fn command_enter_allows_the_tool_for_the_rest_of_the_session() {
    let (mut shell, _data, worktree) = window_against("tests/fixtures/live_write_session.txt");
    shell.set_prompt("write two files");
    press(&mut shell, Key::Enter);

    assert!(
        pump_until(&mut shell, |shell| shell.waiting() == Waiting::Approval),
        "the first write should ask within {TIMEOUT:?}"
    );
    assert!(
        press_with(&mut shell, Key::Enter, command_enter()),
        "Command+Enter is the second approval key"
    );
    assert!(
        pump_until(&mut shell, |shell| turns(shell) == 1),
        "the second write in the same round must not ask again"
    );
    assert_eq!(
        std::fs::read_to_string(worktree.path().join("made.txt")).unwrap(),
        "first"
    );
    assert_eq!(
        std::fs::read_to_string(worktree.path().join("more.txt")).unwrap(),
        "second",
        "the second call ran under the same allowance"
    );

    // The allowance is the session's, not one round's: a later turn writes without asking, and a
    // prompt here would leave the turn blocked until the timeout.
    shell.set_prompt("write a third");
    press(&mut shell, Key::Enter);
    assert!(
        pump_until(&mut shell, |shell| turns(shell) == 2),
        "a later turn's write must not ask again in this session"
    );
    assert_eq!(
        std::fs::read_to_string(worktree.path().join("third.txt")).unwrap(),
        "third"
    );
}

#[test]
fn escape_refuses_the_call_that_is_waiting() {
    let (mut shell, _data, worktree) = window_against("tests/fixtures/live_write.txt");
    shell.set_prompt("write made.txt");
    press(&mut shell, Key::Enter);

    assert!(
        pump_until(&mut shell, |shell| shell.waiting() == Waiting::Approval),
        "the write should ask for approval within {TIMEOUT:?}"
    );
    assert!(press(&mut shell, Key::Escape), "Escape refuses the call");
    assert!(
        pump_until(&mut shell, |shell| turns(shell) == 1),
        "a refusal still ends the turn"
    );
    assert!(
        !worktree.path().join("made.txt").exists(),
        "a refused call does not run"
    );
}
