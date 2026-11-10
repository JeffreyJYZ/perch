//! A card's output: the cap it starts with, and what a toggle swaps it for.

use perch_agent::AgentEvent;
use perch_ui::transcript::{cap_line, expanded_output};
use perch_ui::{COLLAPSED_LINES, Transcript, collapsed_output};

use crate::render::rendered;
use crate::support::{call, numbered};

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
