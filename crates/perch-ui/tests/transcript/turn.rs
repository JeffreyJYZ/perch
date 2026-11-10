//! What one turn draws: its states in words, a streamed paragraph, a note, and its report.

use perch_agent::{AgentEvent, EndReason, Usage};
use perch_tools::Preview;
use perch_ui::transcript::summary_line;
use perch_ui::{Item, Transcript};

use crate::render::rendered;
use crate::support::{call, end, numbered};

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

#[test]
fn a_note_is_a_system_line_of_its_own_with_no_markdown() {
    let mut t = Transcript::default();
    t.note("# Not a heading\n\nThe write tool was refused; **nothing** ran.");

    assert!(
        matches!(t.items().last(), Some(Item::Note(text)) if text.ends_with("**nothing** ran.")),
        "a note is its own item: {:?}",
        t.items()
    );

    // The same words as a reply become a heading and lose their markers; as a note they are text.
    let mut marked = Transcript::default();
    marked.apply(&AgentEvent::TextDelta("# Not a heading\n\n**loud**".into()));
    marked.apply(&end(EndReason::Done));
    assert!(
        rendered(marked).contains("\"role\":\"heading\""),
        "the same words written as a reply are markdown"
    );

    let mut quiet = Transcript::default();
    quiet.note("# Not a heading\n\n**loud**");
    let shown = rendered(quiet);
    assert!(
        shown.contains("# Not a heading") && shown.contains("**loud**"),
        "a note is drawn as it is written: {shown}"
    );
    assert!(
        !shown.contains("\"role\":\"heading\""),
        "no markdown is applied to a note: {shown}"
    );
    assert!(
        !shown.contains("\"Assistant\""),
        "a note belongs to no speaker: {shown}"
    );
}

#[test]
fn a_finished_turn_reports_its_calls_failures_and_tokens() {
    let mut t = Transcript::default();
    t.push_user("fix the build");
    t.apply(&AgentEvent::ToolCall(call("a", "read")));
    t.apply(&AgentEvent::ToolResult {
        id: "a".into(),
        ok: true,
        text: "ok".into(),
    });
    t.apply(&AgentEvent::ToolCall(call("b", "grep")));
    t.apply(&AgentEvent::ToolResult {
        id: "b".into(),
        ok: false,
        text: "no match".into(),
    });
    t.apply(&AgentEvent::TurnEnd {
        reason: EndReason::Done,
        usage: Usage {
            input: 900,
            output: 304,
        },
    });

    let summary = match t.items().last() {
        Some(Item::Summary {
            tools,
            failed,
            tokens,
        }) => summary_line(*tools, *failed, *tokens),
        other => panic!("a turn ends with its report, got {other:?}"),
    };
    assert_eq!(summary, "2 tool calls ran, 1 failed · 1204 tokens.");
    let shown = rendered(t);
    for label in ["2 tool calls", "1 failed", "1204 tokens"] {
        assert!(
            shown.contains(label),
            "the line carries {label} with its number: {shown}"
        );
    }

    assert_eq!(
        summary_line(1, 0, None),
        "1 tool call ran, none failed.",
        "one call reads as one, and no tokens are claimed when none were reported"
    );
    let reply = summary_line(0, 0, Some(12));
    assert!(
        reply.contains("0 tool calls") && reply.contains("12 tokens"),
        "a report with no calls still labels every number: {reply}"
    );
}
