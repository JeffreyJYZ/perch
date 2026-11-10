//! Selection: which groups ⌘A can take, and the seam a markdown reply puts between them.

use perch_agent::AgentEvent;
use perch_ui::Transcript;

use crate::render::{click, runtime, select_all};

#[test]
fn the_plain_turns_are_one_selection_group() {
    // Two plain-text turns and no markdown: the column is one group, so one ⌘A takes it whole.
    let mut t = Transcript::default();
    t.push_user("why is the build red?");
    t.note("the shell exited");
    let mut runtime = runtime(t);
    select_all(&mut runtime);

    let taken = runtime.selected_text().unwrap_or_default();
    assert!(
        taken.contains("why is the build red?"),
        "the user turn is in the group: {taken:?}"
    );
    assert!(
        taken.contains("the shell exited"),
        "a plain system line is in the same group: {taken:?}"
    );
}

#[test]
fn a_markdown_reply_is_a_group_of_the_kit_s_own() {
    // Rune-kit marks each markdown view as its own selection group, so one drag cannot cross
    // from a plain turn into a reply. The column still groups the plain text; this states the
    // seam instead of pretending it is not there.
    let mut t = Transcript::default();
    t.push_user("why is the build red?");
    t.apply(&AgentEvent::TextDelta("Two jobs failed.".into()));
    let mut runtime = runtime(t);

    select_all(&mut runtime);
    assert_eq!(
        runtime.selected_text(),
        None,
        "two groups and nothing clicked: ⌘A picks no one"
    );

    click(&mut runtime, "turn-0-text");
    select_all(&mut runtime);
    let taken = runtime.selected_text().unwrap_or_default();
    assert!(
        taken.contains("why is the build red?"),
        "the clicked group is the plain column: {taken:?}"
    );
    assert!(
        !taken.contains("Two jobs failed."),
        "the reply is the kit's own group: {taken:?}"
    );
}
