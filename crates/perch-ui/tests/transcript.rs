//! The transcript: streamed assistant text grouped into items, the cards tool calls become, and
//! the words the column shows for their state.

use perch_agent::{AgentEvent, EndReason, ToolCall, Usage};
use perch_tools::Preview;
use perch_ui::transcript::{cap_line, expanded_output, summary_line};
use perch_ui::{COLLAPSED_LINES, Item, ToolStatus, Transcript, collapsed_output, transcript_view};
use rune::{Cx, El, Env, Key, KeyEvent, Mods, Runtime};
use serde_json::Value;

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

/// The column laid out once, unread: for a test that presses a key or the pointer.
fn runtime(t: Transcript) -> Runtime<Transcript> {
    let mut runtime = Runtime::new(t, view, (900.0, 700.0), Env::new(true, 2.0));
    runtime.frame(0.0);
    runtime
}

/// The accessibility tree of the column, which carries every word it draws.
fn shaped(t: Transcript) -> Value {
    runtime(t).accessibility_json()
}

/// The node keyed `key`, wherever it sits in the tree.
fn node<'a>(tree: &'a Value, key: &str) -> Option<&'a Value> {
    if tree["key"] == key {
        return Some(tree);
    }
    tree["children"]
        .as_array()?
        .iter()
        .find_map(|child| node(child, key))
}

/// The description a reader hears for the node keyed `key`.
fn described_as(tree: &Value, key: &str) -> Option<String> {
    node(tree, key)
        .and_then(|node| node["description"].as_str())
        .map(str::to_owned)
}

/// ⌘A: the group that was clicked last, else the column's only one.
fn select_all(runtime: &mut Runtime<Transcript>) {
    runtime.key(KeyEvent {
        key: Key::Command('a'),
        mods: Mods {
            cmd: true,
            ..Mods::default()
        },
    });
    runtime.frame(0.0);
}

/// A press and release on the middle of the element keyed `key`.
fn click(runtime: &mut Runtime<Transcript>, key: &str) {
    let rect = runtime
        .elements()
        .into_iter()
        .find(|element| element.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no {key} to click"))
        .rect;
    let (x, y) = (rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5);
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    runtime.frame(0.0);
}

/// The accessibility tree of the column, which carries every word it draws.
fn rendered(t: Transcript) -> String {
    shaped(t).to_string()
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
fn toggling_a_card_swaps_the_cap_for_the_lines_it_hid() {
    let output = numbered(12);
    let collapsed = collapsed_output(&output);
    let expanded = expanded_output(&output);
    let shown = (1..=12)
        .map(|n| format!("line {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    let cap = cap_line(7);

    assert_eq!(expanded, shown, "an expanded card hides nothing");
    assert_eq!(
        collapsed,
        format!(
            "{}\n{cap}",
            shown
                .lines()
                .take(COLLAPSED_LINES)
                .collect::<Vec<_>>()
                .join("\n")
        ),
        "a collapsed card is its first lines and the cap"
    );

    // The two texts differ by exactly the cap line and the lines the cap hid.
    let without_cap = collapsed.replace(&format!("\n{cap}"), "");
    assert_eq!(
        without_cap,
        shown
            .lines()
            .take(COLLAPSED_LINES)
            .collect::<Vec<_>>()
            .join("\n"),
        "the cap line is all that sits beside the lines shown"
    );
    assert_eq!(
        expanded
            .strip_prefix(&without_cap)
            .expect("the collapsed lines are the start of the expanded card"),
        format!(
            "\n{}",
            shown
                .lines()
                .skip(COLLAPSED_LINES)
                .collect::<Vec<_>>()
                .join("\n")
        ),
        "and nothing else was withheld"
    );

    let mut t = Transcript::default();
    t.apply(&AgentEvent::ToolCall(call("a", "read")));
    t.apply(&AgentEvent::ToolResult {
        id: "a".into(),
        ok: true,
        text: output,
    });
    let closed = rendered(t.clone());
    assert!(
        closed.contains("showing first 5") && !closed.contains("line 6"),
        "a card starts collapsed: {closed}"
    );

    t.toggle("a");
    assert!(t.is_expanded("a"));
    let open = rendered(t.clone());
    assert!(
        open.contains("line 12") && !open.contains("showing first 5"),
        "the whole output replaces the cap: {open}"
    );

    t.toggle("a");
    assert!(!t.is_expanded("a"), "a second toggle collapses it again");
    t.toggle("missing");
    assert!(
        !t.is_expanded("missing"),
        "an id that is not a card invents no state"
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
