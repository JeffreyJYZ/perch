//! A question: digits pick or toggle, Enter confirms, t types an answer, Escape cancels.

use perch_ui::shell::Waiting;
use rune::Key;

use crate::harness::{TIMEOUT, answered, press, pump_until, turns, type_key, window_against};

const TWO: Waiting = Waiting::Question {
    options: 2,
    multiple: false,
};
const THREE: Waiting = Waiting::Question {
    options: 3,
    multiple: true,
};

#[test]
fn a_number_key_picks_and_enter_confirms() {
    let (mut shell, _data, _worktree) = window_against("tests/fixtures/live_question.txt");
    shell.set_prompt("go on");
    press(&mut shell, Key::Enter);

    assert!(
        pump_until(&mut shell, |shell| shell.waiting() == TWO),
        "the question should wait for an answer within {TIMEOUT:?}"
    );

    assert!(type_key(&mut shell, "2"), "a digit highlights an option");
    assert_eq!(shell.picked(), Some(1), "the second option is highlighted");
    assert_eq!(shell.waiting(), TWO, "highlighting is not answering");

    assert!(press(&mut shell, Key::Enter), "Enter answers with the pick");
    assert_eq!(shell.picked(), None, "the pick is spent");
    assert!(
        pump_until(&mut shell, |shell| turns(shell) == 1),
        "the turn finishes once the question is answered"
    );
    assert_eq!(
        answered(&shell).as_deref(),
        Some("b"),
        "the model is given the option the user picked"
    );
}

#[test]
fn digits_toggle_each_option_and_enter_sends_them_all() {
    let (mut shell, _data, _worktree) = window_against("tests/fixtures/live_multi.txt");
    shell.set_prompt("pick some");
    press(&mut shell, Key::Enter);

    assert!(
        pump_until(&mut shell, |shell| shell.waiting() == THREE),
        "a question that takes several should wait within {TIMEOUT:?}"
    );

    assert!(type_key(&mut shell, "1"));
    assert!(type_key(&mut shell, "3"));
    assert_eq!(shell.picks(), &[0, 2], "both digits are on");
    assert!(type_key(&mut shell, "1"));
    assert_eq!(shell.picks(), &[2], "a second press turns the option off");
    assert!(type_key(&mut shell, "1"));
    assert_eq!(
        shell.picks(),
        &[2, 0],
        "turning it on again is a new pick, so it comes last"
    );

    assert!(press(&mut shell, Key::Enter), "Enter confirms the set");
    assert!(shell.picks().is_empty(), "the picks are spent");
    assert!(
        pump_until(&mut shell, |shell| turns(shell) == 1),
        "the turn finishes once the set is answered"
    );
    assert_eq!(
        answered(&shell).as_deref(),
        Some("c\na"),
        "the model is given every option picked, in the order they were picked"
    );
}

#[test]
fn enter_with_nothing_picked_is_refused_in_words() {
    let (mut shell, _data, _worktree) = window_against("tests/fixtures/live_multi.txt");
    shell.set_prompt("pick some");
    press(&mut shell, Key::Enter);

    assert!(
        pump_until(&mut shell, |shell| shell.waiting() == THREE),
        "the question should wait within {TIMEOUT:?}"
    );
    assert!(
        press(&mut shell, Key::Enter),
        "Enter is still the confirm key"
    );
    assert!(
        shell
            .answer_note()
            .is_some_and(|note| note.contains("nothing picked")),
        "the refusal says why: {:?}",
        shell.answer_note()
    );
    assert_eq!(shell.waiting(), THREE, "the question is still waiting");
    for _ in 0..4 {
        shell.pump(std::time::Duration::from_millis(5));
    }
    assert_eq!(turns(&shell), 0, "nothing was sent on an empty answer");
    assert_eq!(answered(&shell), None, "the model was told nothing");
}

#[test]
fn t_opens_a_field_and_enter_sends_what_was_typed() {
    let (mut shell, _data, _worktree) = window_against("tests/fixtures/live_question.txt");
    shell.set_prompt("go on");
    press(&mut shell, Key::Enter);

    assert!(
        pump_until(&mut shell, |shell| shell.waiting() == TWO),
        "the question should wait within {TIMEOUT:?}"
    );
    assert!(type_key(&mut shell, "t"), "t opens the answer field");
    assert!(shell.typing(), "the card is showing its field");
    assert!(shell.picks().is_empty(), "t is not a pick");

    shell.set_answer_text("neither of them");
    assert!(
        press(&mut shell, Key::Enter),
        "Enter sends the typed answer"
    );
    assert!(pump_until(&mut shell, |shell| turns(shell) == 1));
    assert_eq!(
        answered(&shell).as_deref(),
        Some("neither of them"),
        "the model is given the text, not an option"
    );
}

#[test]
fn escape_goes_back_to_the_options_without_answering() {
    let (mut shell, _data, _worktree) = window_against("tests/fixtures/live_question.txt");
    shell.set_prompt("go on");
    press(&mut shell, Key::Enter);

    assert!(pump_until(&mut shell, |shell| shell.waiting() == TWO));
    assert!(type_key(&mut shell, "1"), "an option is picked first");
    assert!(type_key(&mut shell, "t"), "then the field is opened");
    shell.set_answer_text("half-typed");

    assert!(press(&mut shell, Key::Escape), "Escape leaves the field");
    assert!(!shell.typing(), "the card is back to its options");
    assert_eq!(shell.answer_text(), "", "the half-typed text is gone");
    assert_eq!(shell.picks(), &[0], "the pick survives going back");
    assert_eq!(shell.waiting(), TWO, "nothing was answered");

    assert!(press(&mut shell, Key::Enter), "Enter confirms the pick");
    assert!(pump_until(&mut shell, |shell| turns(shell) == 1));
    assert_eq!(answered(&shell).as_deref(), Some("a"));
}

#[test]
fn a_question_with_no_options_opens_the_field_on_enter() {
    let (mut shell, _data, _worktree) = window_against("tests/fixtures/live_ask_free.txt");
    shell.set_prompt("how?");
    press(&mut shell, Key::Enter);

    let free = Waiting::Question {
        options: 0,
        multiple: false,
    };
    assert!(
        pump_until(&mut shell, |shell| shell.waiting() == free),
        "a free-text question should wait within {TIMEOUT:?}"
    );
    assert!(press(&mut shell, Key::Enter), "Enter opens the field");
    assert!(
        shell.typing(),
        "there is nothing to pick, so the field is the answer"
    );
    assert!(
        press(&mut shell, Key::Enter),
        "Enter on the empty field is consumed"
    );
    assert!(
        shell
            .answer_note()
            .is_some_and(|note| note.contains("nothing typed")),
        "{:?}",
        shell.answer_note()
    );

    shell.set_answer_text("with a cancel button");
    assert!(press(&mut shell, Key::Enter));
    assert!(pump_until(&mut shell, |shell| turns(shell) == 1));
    assert_eq!(answered(&shell).as_deref(), Some("with a cancel button"));
}
