//! A card opened: more than the collapsed cap, the cut the pane makes, and the truncation the tool
//! had already made when it recorded the text.

use perch_agent::AgentEvent;
use perch_ui::transcript::{OPENED_LINES, cut_line, opened_output};
use perch_ui::{Item, Transcript};

use crate::render::{click, rendered, runtime, shaped};
use crate::support::{call, numbered};

/// A transcript holding one finished card whose output is `output`.
fn with_output(output: &str) -> Transcript {
    let mut t = Transcript::default();
    t.apply(&AgentEvent::ToolCall(call("a", "read")));
    t.apply(&AgentEvent::ToolResult {
        id: "a".into(),
        ok: true,
        text: output.to_string(),
    });
    t
}

/// The card's own text, as a reader moving by controls hears it.
fn card_words(t: &Transcript) -> String {
    let tree = shaped(t.clone());
    let card = crate::render::node(&tree, "tool-0").expect("the card is a node");
    card.to_string()
}

#[test]
fn a_press_on_the_header_opens_more_than_the_card_shows() {
    let t = with_output(&numbered(12));
    let mut runtime = runtime(t.clone());

    assert!(
        card_words(&t).contains("showing first 5"),
        "a closed card elides: {}",
        card_words(&t)
    );
    click(&mut runtime, "tool-0-head");
    let opened = runtime.accessibility_json().to_string();
    assert!(
        opened.contains("line 12") && !opened.contains("showing first 5"),
        "the header press draws the whole output: {opened}"
    );
    assert!(
        opened.contains("12 lines"),
        "and the opened block names what it read: {opened}"
    );

    // The same press shuts it, and no state was invented behind the card.
    click(&mut runtime, "tool-0-head");
    let closed = runtime.accessibility_json().to_string();
    assert!(
        closed.contains("showing first 5"),
        "a second press collapses it again: {closed}"
    );
}

#[test]
fn an_opened_block_names_the_cut_the_pane_makes() {
    let output = numbered(OPENED_LINES + 20);
    let mut t = with_output(&output);
    assert!(t.toggle("a"));

    let words = opened_output(&output, output.trim_end().len());
    assert!(
        words.contains(&cut_line(20, OPENED_LINES + 20, output.trim_end().len())),
        "the drawing says how many lines it left out: {words}"
    );
    assert!(
        !words.contains(&format!("line {}\n", OPENED_LINES + 1)),
        "and those lines are not drawn"
    );

    let tree = rendered(t);
    assert!(
        tree.contains("20 more lines not drawn"),
        "the cut is on screen, not only in the string: {tree}"
    );
    assert!(
        tree.contains(&format!("{} lines, ", OPENED_LINES + 20)),
        "with the size the tool recorded: {tree}"
    );
}

#[test]
fn a_truncation_the_tool_made_is_said_with_the_size_it_kept() {
    let output = format!("{}\n[truncated 44000 bytes]", numbered(6));
    let mut t = with_output(&output);
    assert!(t.toggle("a"));

    let tree = rendered(t.clone());
    assert!(
        tree.contains("the tool cut this when it ran"),
        "a short block does not read as the whole answer: {tree}"
    );
    assert!(
        tree.contains("44000 bytes"),
        "the tool's own marker is quoted: {tree}"
    );
    let kept = output.trim_end().len();
    assert!(
        tree.contains(&format!("{kept} bytes kept")),
        "and the size that was recorded is stated: {tree}"
    );

    // A card the tool cut is still opened: the cut is a fact about the text, not a refusal.
    assert!(t.is_expanded("a"));
    let held = t.items().iter().find_map(|item| match item {
        Item::Tool { output: held, .. } => Some(held),
        _ => None,
    });
    assert_eq!(
        held,
        Some(&output),
        "the item holds the recorded block exactly"
    );
}

#[test]
fn a_card_with_no_output_says_so_instead_of_opening() {
    let mut running = Transcript::default();
    running.apply(&AgentEvent::ToolCall(call("a", "bash")));
    assert!(!running.toggle("a"), "a running call has nothing to open");
    let tree = rendered(running);
    assert!(
        tree.contains("no output yet — the call is still running."),
        "the card says why in words: {tree}"
    );

    let mut refused = Transcript::default();
    refused.apply(&AgentEvent::ToolCall(call("a", "bash")));
    refused.apply(&AgentEvent::ToolResult {
        id: "a".into(),
        ok: false,
        text: String::new(),
    });
    assert!(!refused.toggle("a"), "a refusal has nothing to open either");
    let tree = rendered(refused);
    assert!(
        tree.contains("no output — the call returned nothing."),
        "and is said in words rather than drawn as an empty block: {tree}"
    );
}

#[test]
fn the_turn_continuing_shuts_an_opened_card() {
    let mut t = with_output(&numbered(12));
    t.toggle("a");
    t.apply(&AgentEvent::ToolResult {
        id: "a".into(),
        ok: true,
        text: numbered(14),
    });
    assert!(
        !t.is_expanded("a"),
        "a result arriving under a reader shuts the block it was reading"
    );

    t.toggle("a");
    t.push_user("next question");
    assert!(
        !t.is_expanded("a"),
        "and another turn starting shuts it too, so the block never outlives its turn"
    );
}
