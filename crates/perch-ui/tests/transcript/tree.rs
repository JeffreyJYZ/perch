//! Each turn as one node: who spoke, what state it is in, and when it stops saying so.

use perch_agent::{AgentEvent, EndReason};
use perch_ui::Transcript;

use crate::render::{described_as, node, shaped};
use crate::support::{call, end};

#[test]
fn every_turn_is_one_node_that_names_its_speaker_and_state() {
    let mut t = Transcript::default();
    t.push_user("why is the build red?");
    t.apply(&AgentEvent::TextDelta("Two jobs failed.".into()));
    let tree = shaped(t.clone());

    let user = node(&tree, "turn-0").expect("the user turn is a node of its own");
    assert_eq!(
        user["role"], "group",
        "a turn is one readable group: {user}"
    );
    assert_eq!(user["name"], "You", "a turn says who spoke: {user}");

    let reply = node(&tree, "turn-1").expect("the reply is a node of its own");
    assert_eq!(reply["role"], "group");
    assert_eq!(reply["name"], "Assistant");
    assert_eq!(
        reply["description"], "streaming",
        "a live reply says it is still arriving: {reply}"
    );

    // A tool call closes the paragraph above it and adds a card of its own.
    t.apply(&AgentEvent::ToolCall(call("a", "read")));
    let card = shaped(t);
    let card = node(&card, "tool-2").expect("the card is a node of its own");
    assert_eq!(
        card["role"], "button",
        "a card is one pressable node: {card}"
    );
    let said = card["name"].as_str().unwrap_or_default();
    assert!(
        said.contains("read"),
        "the card names the call that ran: {said}"
    );
    assert_eq!(
        card["description"], "running",
        "and where it got to, in words: {card}"
    );
}

#[test]
fn a_streaming_turn_stops_saying_so_and_a_cancelled_one_says_cancelled() {
    let mut t = Transcript::default();
    t.apply(&AgentEvent::TextDelta("half a thought".into()));
    let live = shaped(t.clone());
    assert_eq!(described_as(&live, "turn-0").as_deref(), Some("streaming"));

    t.apply(&end(EndReason::Done));
    let done = shaped(t.clone());
    assert_eq!(
        described_as(&done, "turn-0"),
        None,
        "a finished turn says nothing about arriving"
    );
    assert!(
        !done.to_string().contains("streaming"),
        "and the word is gone from the tree: {done}"
    );

    let mut cancelled = Transcript::default();
    cancelled.apply(&AgentEvent::TextDelta("half a thought".into()));
    cancelled.apply(&end(EndReason::Cancelled));
    let stopped = shaped(cancelled);
    assert_eq!(
        described_as(&stopped, "turn-0").as_deref(),
        Some("cancelled"),
        "the turn itself says it was cancelled, not only the system line"
    );
    assert!(
        !stopped.to_string().contains("streaming"),
        "a cancelled turn never claims to be running: {stopped}"
    );
}
