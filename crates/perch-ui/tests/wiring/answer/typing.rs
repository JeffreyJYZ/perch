//! What the presses do to the card: `t` types into it, Enter sends that text, Escape goes back.

use rune::Key;

use super::{MULTI, QUESTION, find, laid_out, press, tree, typed};

#[test]
fn t_types_into_the_card_and_enter_sends_what_was_typed() {
    let mut runtime = laid_out(QUESTION);
    typed(&mut runtime, "t");
    assert!(runtime.state.typing(), "t opens the answer field");
    assert!(
        runtime.state.picks().is_empty(),
        "t is not an option, so it picks nothing"
    );
    assert!(
        find(&runtime.accessibility_json(), "answer-field").is_some(),
        "the field is in the card: {}",
        tree(&runtime)
    );

    typed(&mut runtime, "h");
    typed(&mut runtime, "i");
    assert_eq!(
        runtime.state.answer_text(),
        "hi",
        "characters land in the card's field, not in the composer"
    );
    assert_eq!(runtime.state.prompt_text(), "", "the composer is untouched");

    press(&mut runtime, Key::Enter);
    assert_eq!(
        runtime.state.live().unwrap().answered.as_deref(),
        Some("hi"),
        "Enter sends the typed answer"
    );
}

#[test]
fn escape_goes_back_to_the_picks_without_answering() {
    let mut runtime = laid_out(MULTI);
    typed(&mut runtime, "1");
    typed(&mut runtime, "3");
    assert_eq!(runtime.state.picks(), &[0, 2], "two options are picked");
    typed(&mut runtime, "t");
    typed(&mut runtime, "x");
    assert_eq!(runtime.state.answer_text(), "x");

    press(&mut runtime, Key::Escape);
    assert!(!runtime.state.typing(), "Escape closes the field");
    assert_eq!(runtime.state.answer_text(), "", "and empties it");
    assert_eq!(
        runtime.state.picks(),
        &[0, 2],
        "the picks made before typing survive"
    );
    assert_eq!(
        runtime.state.live().unwrap().answered,
        None,
        "Escape answers nothing"
    );
    assert_eq!(
        runtime.state.live().unwrap().waiting,
        MULTI,
        "the question is still waiting"
    );
}
