//! Thinking arrives in its own item, and the answer that follows never continues it.

use perch_agent::AgentEvent;
use perch_ui::{Item, Transcript};

use crate::render::rendered;

/// The thought and the answer, in the order the column holds them.
fn items(t: &Transcript) -> Vec<String> {
    t.items()
        .iter()
        .map(|item| match item {
            Item::Reasoning { text } => format!("think:{text}"),
            Item::Assistant { text } => format!("say:{text}"),
            other => format!("other:{other:?}"),
        })
        .collect()
}

#[test]
fn deltas_of_one_thought_join_and_the_answer_starts_a_new_item() {
    let mut t = Transcript::default();
    t.apply(&AgentEvent::ReasoningDelta("Let me think".into()));
    t.apply(&AgentEvent::ReasoningDelta(" about it".into()));
    t.apply(&AgentEvent::TextDelta("The answer.".into()));

    assert_eq!(
        items(&t),
        ["think:Let me think about it", "say:The answer."],
        "a thought is one item, apart from the reply"
    );
    let shown = rendered(t);
    assert!(
        shown.contains("Thinking") && shown.contains("Let me think about it"),
        "the block is labelled and holds the thought:\n{shown}"
    );
}

#[test]
fn thinking_after_a_reply_opens_its_own_block_and_the_next_reply_a_new_one() {
    let mut t = Transcript::default();
    t.apply(&AgentEvent::TextDelta("First.".into()));
    t.apply(&AgentEvent::ReasoningDelta("Reconsidering.".into()));
    t.apply(&AgentEvent::TextDelta("Second.".into()));

    assert_eq!(
        items(&t),
        ["say:First.", "think:Reconsidering.", "say:Second."],
        "no paragraph carries two voices"
    );
}

#[test]
fn a_thought_alone_draws_nothing_as_an_answer() {
    let mut t = Transcript::default();
    t.apply(&AgentEvent::ReasoningDelta("Only thinking.".into()));

    assert!(
        !t.items()
            .iter()
            .any(|item| matches!(item, Item::Assistant { .. })),
        "a thought is never an assistant reply: {:?}",
        t.items()
    );
}
