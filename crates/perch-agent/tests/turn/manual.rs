//! `/compact` as a control: the conversation shrinks now, and one with nothing to drop says so.
//!
//! The request is not a turn: it answers with one `Compacted` and no `TurnEnd`, because nothing was
//! asked of the model and nothing was streamed. A caller waiting for it is never left silent.

use perch_agent::compact::now;
use perch_agent::{AgentEvent, Message};

use crate::harness::{agent_at, collect};

/// A conversation with two finished exchanges, newest last.
fn two_exchanges() -> Vec<Message> {
    vec![
        Message::system("you are a test"),
        Message::user("first"),
        Message::assistant("hello there"),
        Message::user("second"),
        Message::assistant("still here"),
    ]
}

#[test]
fn compacting_drops_the_oldest_exchange_and_keeps_the_system_and_the_newest() {
    let mut history = two_exchanges();

    let dropped = now(&mut history).expect("two exchanges can be halved");

    assert_eq!(dropped.exchanges, 1, "one whole exchange was left out");
    assert_eq!(dropped.messages, 2, "and its two messages with it");
    assert_eq!(
        history
            .iter()
            .map(|message| message.content.as_deref().unwrap_or_default())
            .collect::<Vec<_>>(),
        ["you are a test", "second", "still here"],
        "the system message and the newest exchange survive whole"
    );
}

#[test]
fn a_conversation_with_nothing_older_to_drop_reports_none_and_loses_nothing() {
    let mut history = vec![
        Message::system("you are a test"),
        Message::user("only"),
        Message::assistant("answer"),
    ];

    assert!(now(&mut history).is_none(), "there is no older exchange");
    assert_eq!(history.len(), 3, "a refused compaction drops nothing");
}

#[test]
fn the_agent_answers_a_compaction_with_its_count_and_no_turn_end() {
    // No request is made, so the address is never reached: this test is about the control path.
    let (handle, events) = agent_at("http://127.0.0.1:1/v1");

    handle.compact();

    let answer = events
        .recv_timeout(crate::harness::EVENT_TIMEOUT)
        .expect("a compaction answers");
    assert_eq!(
        answer,
        AgentEvent::Compacted {
            dropped: 0,
            exchanges: 0
        },
        "a conversation with no exchanges has nothing to leave out"
    );
    let seen = collect(&events);
    assert!(
        seen.ends().is_empty(),
        "a compaction is not a turn, so it ends none: {:?}",
        seen.events
    );
}
