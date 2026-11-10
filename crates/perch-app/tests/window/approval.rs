//! A waiting call: Enter allows it, Escape refuses it.

use perch_ui::shell::Waiting;
use rune::Key;

use crate::harness::{TIMEOUT, press, pump_until, turns, window_against};

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
