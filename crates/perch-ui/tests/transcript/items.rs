//! The items a transcript is made of: deltas, matching cards, statuses and the turn's end.

use perch_agent::{AgentEvent, EndReason};
use perch_ui::{Item, ToolStatus, Transcript};

use crate::support::{call, end};

#[test]
fn deltas_append_to_the_open_assistant_item() {
    let mut t = Transcript::default();
    t.apply(&AgentEvent::TextDelta("Hel".into()));
    t.apply(&AgentEvent::TextDelta("lo".into()));
    assert!(matches!(&t.items()[0], Item::Assistant { text } if text == "Hello"));
}

#[test]
fn a_tool_result_updates_the_matching_card_only() {
    let mut t = Transcript::default();
    t.apply(&AgentEvent::ToolCall(call("a", "read")));
    t.apply(&AgentEvent::ToolCall(call("b", "grep")));
    t.apply(&AgentEvent::ToolResult {
        id: "b".into(),
        ok: false,
        text: "no match".into(),
    });
    let statuses: Vec<_> = t
        .items()
        .iter()
        .filter_map(|i| match i {
            Item::Tool { id, status, .. } => Some((id.as_str(), *status)),
            _ => None,
        })
        .collect();
    assert_eq!(
        statuses,
        [("a", ToolStatus::Running), ("b", ToolStatus::Failed)]
    );
}

#[test]
fn every_status_is_a_word() {
    assert_eq!(
        [
            ToolStatus::Running.words(),
            ToolStatus::Ok.words(),
            ToolStatus::Failed.words(),
            ToolStatus::Cancelled.words(),
            ToolStatus::ApprovalNeeded.words(),
        ],
        ["running", "done", "failed", "cancelled", "approval needed"]
    );
}

#[test]
fn a_turn_that_ends_closes_the_item_and_cancels_what_was_running() {
    let mut t = Transcript::default();
    t.apply(&AgentEvent::ToolCall(call("a", "bash")));
    t.apply(&end(EndReason::Cancelled));
    t.apply(&AgentEvent::TextDelta("after".into()));

    let statuses: Vec<_> = t
        .items()
        .iter()
        .filter_map(|i| match i {
            Item::Tool { status, .. } => Some(*status),
            _ => None,
        })
        .collect();
    assert_eq!(
        statuses,
        [ToolStatus::Cancelled],
        "a turn ends in a definite state"
    );
    assert!(
        matches!(t.items().last(), Some(Item::Assistant { text }) if text == "after"),
        "text after the turn opens a new item: {:?}",
        t.items()
    );
}
