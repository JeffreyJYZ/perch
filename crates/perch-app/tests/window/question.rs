//! A question: a digit highlights an option and Enter answers with it.

use perch_ui::Item;
use perch_ui::shell::Waiting;
use rune::Key;

use crate::harness::{TIMEOUT, press, pump_until, turns, type_key, window_against};

#[test]
fn a_number_key_picks_and_enter_confirms() {
    let (mut shell, _data, _worktree) = window_against("tests/fixtures/live_question.txt");
    shell.set_prompt("go on");
    press(&mut shell, Key::Enter);

    assert!(
        pump_until(&mut shell, |shell| shell.waiting()
            == Waiting::Question { options: 2 }),
        "the question should wait for an answer within {TIMEOUT:?}"
    );

    assert!(type_key(&mut shell, "2"), "a digit highlights an option");
    assert_eq!(shell.picked(), Some(1), "the second option is highlighted");
    assert_eq!(
        shell.waiting(),
        Waiting::Question { options: 2 },
        "highlighting is not answering"
    );

    assert!(press(&mut shell, Key::Enter), "Enter answers with the pick");
    assert_eq!(shell.picked(), None, "the pick is spent");
    assert!(
        pump_until(&mut shell, |shell| turns(shell) == 1),
        "the turn finishes once the question is answered"
    );
    let output = shell
        .transcript()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::Tool { output, .. } => Some(output.clone()),
            _ => None,
        })
        .expect("the question's call is a card");
    assert_eq!(output, "b", "the model is given the option the user picked");
}
