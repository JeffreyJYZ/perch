//! ⌘F over the conversation: the bar's field takes the keyboard on its first frame, the scan
//! reads the visible words of replies and tool cards, Up and Down move the column with them, and
//! Escape closes the bar and leaves the reader on the hit.

use perch_agent::AgentEvent;
use rune::Key;

use crate::harness::{
    call, chord, header, in_view, node, over, press, scroll_offset, session, type_text, window,
    words,
};

#[test]
fn the_chord_opens_a_field_that_is_focused_on_the_first_frame() {
    let mut runtime = over(|shell| shell.transcript_mut().push_user("a needle in the text"));
    chord(&mut runtime, 'f');
    assert!(runtime.state.find_open(), "⌘F opened the bar");
    let tree = runtime.accessibility_json();
    assert!(
        node(&tree, "find-query").is_some(),
        "the field is drawn on the frame it opened"
    );

    // The very next keystroke reaches the field: nothing has to be clicked first.
    type_text(&mut runtime, "needle");
    assert_eq!(runtime.state.find_query_text(), "needle");
    assert_eq!(
        runtime.state.prompt_text(),
        "",
        "the composer received none of the keys"
    );

    // A second chord closes it, so the same press opens and shuts the bar.
    chord(&mut runtime, 'f');
    assert!(!runtime.state.find_open());
}

#[test]
fn a_reply_and_a_tool_cards_visible_words_are_both_found() {
    let output = (1..=12)
        .map(|line| {
            if line == 9 {
                "line 9: special token".to_string()
            } else {
                format!("line {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut runtime = over(move |shell| {
        let transcript = shell.transcript_mut();
        transcript.push_user("why is the build red?");
        transcript.apply(&AgentEvent::TextDelta(
            "The failure is a missing link in the plan.".into(),
        ));
        transcript.apply(&AgentEvent::ToolCall(call("a", "grep")));
        transcript.apply(&AgentEvent::ToolResult {
            id: "a".into(),
            ok: true,
            text: output,
        });
    });

    chord(&mut runtime, 'f');
    type_text(&mut runtime, "failure");
    assert_eq!(runtime.state.find_hits().len(), 1, "the reply is scanned");
    assert!(words(&runtime).contains("1 of 1"), "{}", words(&runtime));

    press(&mut runtime, Key::Escape);
    chord(&mut runtime, 'f');
    type_text(&mut runtime, "special token");
    assert_eq!(
        runtime.state.find_hits().len(),
        1,
        "a tool card's output is scanned past the lines a collapsed card shows"
    );
    assert_eq!(runtime.state.find_mark(), Some(2));

    press(&mut runtime, Key::Escape);
    chord(&mut runtime, 'f');
    type_text(&mut runtime, "grep");
    assert_eq!(
        runtime.state.find_hits().len(),
        1,
        "a card's header names what ran"
    );
}

#[test]
fn down_and_up_move_between_hits_and_the_column_follows() {
    let mut runtime = over(|shell| {
        for turn in 0..40 {
            let words = if turn == 0 || turn == 30 {
                format!("needle {turn}")
            } else {
                format!("turn {turn}")
            };
            shell.transcript_mut().push_user(words);
        }
    });

    chord(&mut runtime, 'f');
    type_text(&mut runtime, "needle");
    assert_eq!(runtime.state.find_hits().len(), 2);
    assert_eq!(runtime.state.find_mark(), Some(0));
    assert!(in_view(&runtime, "find-hit-0"), "the first hit is shown");
    let first = header(&runtime);

    press(&mut runtime, Key::Down);
    assert_eq!(runtime.state.find_index(), 1);
    assert_eq!(runtime.state.find_mark(), Some(30));
    assert!(
        in_view(&runtime, "find-hit-30"),
        "the column scrolled to the second hit"
    );
    let second = header(&runtime);
    assert_ne!(first, second, "the column really moved: {first:?}");

    // Down wraps to the first hit, and Up wraps back to the last.
    press(&mut runtime, Key::Down);
    assert_eq!(runtime.state.find_index(), 0);
    assert!(in_view(&runtime, "find-hit-0"));
    press(&mut runtime, Key::Up);
    assert_eq!(runtime.state.find_index(), 1);
    assert_eq!(runtime.state.find_mark(), Some(30));
}

#[test]
fn a_miss_and_an_empty_transcript_say_so() {
    let mut runtime = over(|shell| shell.transcript_mut().push_user("something else"));
    chord(&mut runtime, 'f');
    type_text(&mut runtime, "zzzz");
    assert!(runtime.state.find_hits().is_empty());
    assert!(
        words(&runtime).contains("no matches"),
        "a miss is said in words: {}",
        words(&runtime)
    );

    let mut empty = window(vec![session("s", "/work/s", "dev")], 0);
    chord(&mut empty, 'f');
    assert!(
        words(&empty).contains("the transcript is empty"),
        "an empty conversation is said to be empty, not 0 of 0: {}",
        words(&empty)
    );
}

#[test]
fn closing_leaves_the_scroll_where_it_was() {
    let mut runtime = over(|shell| {
        for turn in 0..40 {
            let words = if turn == 30 {
                "needle here".to_string()
            } else {
                format!("turn {turn}")
            };
            shell.transcript_mut().push_user(words);
        }
    });

    chord(&mut runtime, 'f');
    type_text(&mut runtime, "needle");
    let found = scroll_offset(&runtime);
    assert!(found > 0.0, "the search moved the column to the hit");

    press(&mut runtime, Key::Escape);
    assert!(!runtime.state.find_open(), "Escape closes the bar");
    // The bar's own height rejoins the transcript, so the line count above changes by the two
    // lines it gives back. What must not move is the column: the reader stays on the same text.
    assert_eq!(
        scroll_offset(&runtime),
        found,
        "the column stayed exactly where the search left it"
    );
}
