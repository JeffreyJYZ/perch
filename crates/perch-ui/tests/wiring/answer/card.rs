//! What the card says: every key a wait answers, in words.

use perch_ui::shell::Waiting;

use super::{MULTI, QUESTION, laid_out, tree};

#[test]
fn an_approval_card_says_which_key_allows_allows_for_the_session_and_refuses() {
    let words = tree(&laid_out(Waiting::Approval));
    assert!(words.contains("Enter allows this call once."), "{words}");
    assert!(
        words.contains("Command+Enter allows this tool for the rest of the session."),
        "the second approval key is named in words, not a glyph only: {words}"
    );
    assert!(words.contains("Escape refuses it"), "{words}");
}

#[test]
fn a_question_card_says_which_keys_pick_confirm_type_and_cancel() {
    let words = tree(&laid_out(QUESTION));
    assert!(words.contains("Number keys pick an option."), "{words}");
    assert!(
        words.contains("Enter confirms the option picked."),
        "{words}"
    );
    assert!(words.contains("Press t to type your own answer"), "{words}");
    assert!(words.contains("Escape to cancel"), "{words}");
    assert!(
        !words.contains("Command+Enter"),
        "an approval's key list is not a question's: {words}"
    );
}

#[test]
fn a_question_that_takes_several_says_the_numbers_toggle() {
    let words = tree(&laid_out(MULTI));
    assert!(
        words.contains("Number keys toggle an option on and off."),
        "{words}"
    );
    assert!(
        words.contains("Enter confirms every option picked."),
        "{words}"
    );
}
