//! `/compact` typed into the composer: taken as a control, never sent as a prompt.

use perch_ui::Shell;

use crate::fake::Fake;

#[test]
fn a_typed_compact_asks_for_a_compaction_and_sends_no_prompt() {
    let mut shell = Shell::new(Vec::new(), 0).with_live(Fake::default());

    shell.set_prompt("/compact");
    assert!(shell.submit(), "the press is taken");

    let fake = shell.live().expect("the window kept its app");
    assert_eq!(fake.compacted, 1, "the window asked the session to compact");
    assert_eq!(
        fake.prompted, None,
        "`/compact` is not a user message and must never reach the model as one"
    );
    assert_eq!(shell.prompt_text(), "", "the box is emptied by the press");
}

#[test]
fn only_the_bare_word_is_the_control() {
    let mut shell = Shell::new(Vec::new(), 0).with_live(Fake::default());

    shell.set_prompt("/compact this please");
    shell.submit();

    let fake = shell.live().expect("the window kept its app");
    assert_eq!(
        fake.prompted.as_deref(),
        Some("/compact this please"),
        "anything but the bare command is an ordinary prompt"
    );
    assert_eq!(fake.compacted, 0);
}
