//! The transcript: streamed assistant text grouped into items, the cards tool calls become, and
//! the words the column shows for their state.

use perch_agent::{AgentEvent, EndReason, ToolCall, Usage};
use perch_tools::Preview;
use perch_ui::{COLLAPSED_LINES, Item, ToolStatus, Transcript, collapsed_output, transcript_view};
use rune::{Cx, El, Env, Runtime};

fn call(id: &str, name: &str) -> ToolCall {
    ToolCall {
        id: id.to_string(),
        name: name.to_string(),
        args: serde_json::json!({ "path": "src/main.rs" }),
    }
}

/// `lines` numbered lines, each with its newline: longer than a collapsed card shows.
fn numbered(lines: usize) -> String {
    (1..=lines).map(|n| format!("line {n}\n")).collect()
}

fn end(reason: EndReason) -> AgentEvent {
    AgentEvent::TurnEnd {
        reason,
        usage: Usage::default(),
    }
}

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
fn a_collapsed_tool_card_names_its_line_cap() {
    let card = collapsed_output(&numbered(12));

    assert!(card.contains("line 5"), "the first lines show: {card}");
    assert!(
        !card.contains("line 6"),
        "the rest is behind the cap: {card}"
    );
    assert!(
        card.contains("showing first 5"),
        "the cap is stated in the text: {card}"
    );
    assert!(
        card.contains("7 more lines"),
        "the text says what is hidden: {card}"
    );
    assert_eq!(
        card.lines().count(),
        COLLAPSED_LINES + 1,
        "five lines and the cap: {card}"
    );
    assert_eq!(
        collapsed_output("a\nb\n"),
        "a\nb",
        "a short result claims nothing"
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

fn view(t: &Transcript, _: &mut Cx<Transcript>) -> El {
    transcript_view(t)
}

/// The accessibility tree of the column, which carries every word it draws.
fn rendered(t: Transcript) -> String {
    let mut runtime = Runtime::new(t, view, (900.0, 700.0), Env::new(true, 2.0));
    runtime.frame(0.0);
    runtime.accessibility_json().to_string()
}

#[test]
fn the_column_says_each_state_in_words_and_states_the_output_cap() {
    let mut t = Transcript::default();
    t.push_user("why is the build red?");
    t.apply(&AgentEvent::TextDelta("Two jobs failed.\n\n".into()));
    t.apply(&AgentEvent::ToolCall(call("a", "read")));
    t.apply(&AgentEvent::ToolCall(call("b", "grep")));
    t.apply(&AgentEvent::ToolResult {
        id: "b".into(),
        ok: false,
        text: numbered(12),
    });
    t.apply(&AgentEvent::ToolCall(call("c", "edit")));
    t.request_approval("c", Preview::Command("cargo test".into()));

    let live = rendered(t.clone());
    for word in [
        "You",
        "running",
        "failed",
        "approval needed",
        "showing first 5",
    ] {
        assert!(live.contains(word), "the tree is missing {word}:\n{live}");
    }
    assert!(
        !live.contains("line 6"),
        "the cap holds in the view:\n{live}"
    );

    t.apply(&end(EndReason::Cancelled));
    assert!(
        rendered(t).contains("cancelled"),
        "a cancelled turn still shows a running card as cancelled"
    );
}

#[test]
fn the_open_turn_renders_markdown_as_a_stream_not_as_source() {
    // An unclosed emphasis is what a stream is for: it draws styled, never as its markers.
    let mut open = Transcript::default();
    open.apply(&AgentEvent::TextDelta("**Emphasis".into()));
    let streaming = rendered(open);
    assert!(streaming.contains("Emphasis"), "{streaming}");
    assert!(
        !streaming.contains("**"),
        "an open span never flashes its markers: {streaming}"
    );

    let mut closed = Transcript::default();
    closed.apply(&AgentEvent::TextDelta("**Emphasis".into()));
    closed.apply(&end(EndReason::Done));
    let whole = rendered(closed);
    assert!(
        whole.contains("**"),
        "a finished turn is taken as written: {whole}"
    );
}
