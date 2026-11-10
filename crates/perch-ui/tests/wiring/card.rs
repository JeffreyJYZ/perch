//! A tool card in the window: Enter and Space open it only once it is focused.

use perch_agent::{AgentEvent, ToolCall};
use perch_ui::Shell;
use rune::{Key, KeyEvent, Mods};

use crate::harness::{click, enter, meta, window, words};

#[test]
fn enter_opens_the_focused_tool_card_and_opens_nothing_with_no_card_focused() {
    let mut shell = Shell::idle(vec![meta("work")], 0);
    {
        let transcript = shell.transcript_mut();
        transcript.apply(&AgentEvent::ToolCall(ToolCall {
            id: "call-1".to_string(),
            name: "read".to_string(),
            args: serde_json::json!({ "path": "src/main.rs" }),
        }));
        transcript.apply(&AgentEvent::ToolResult {
            id: "call-1".to_string(),
            ok: true,
            text: (1..=12).map(|n| format!("line {n}\n")).collect(),
        });
    }
    let mut runtime = window(shell);
    assert!(
        words(&runtime).contains("showing first 5"),
        "a card starts collapsed: {}",
        words(&runtime)
    );

    // Nothing is focused, so Enter is the composer's: it opens no card.
    runtime.key(enter());
    runtime.frame(0.0);
    assert!(
        !runtime.state.transcript().is_expanded("call-1"),
        "Enter with no card focused opens nothing"
    );

    // A press on the card focuses it, and then Enter is the card's.
    click(&mut runtime, "tool-0");
    runtime.key(enter());
    runtime.frame(0.0);
    assert!(
        runtime.state.transcript().is_expanded("call-1"),
        "Enter on the focused card opens it: {}",
        words(&runtime)
    );
    let opened = words(&runtime);
    assert!(
        opened.contains("line 12") && !opened.contains("showing first 5"),
        "the whole output replaces the cap: {opened}"
    );

    runtime.key(enter());
    runtime.frame(0.0);
    assert!(
        !runtime.state.transcript().is_expanded("call-1"),
        "a second Enter closes it again"
    );

    // Space is Rune's own activation key for a focused control, and a card has no press handler;
    // the root's key handler runs first, so Space reaches the same toggle the window map does.
    runtime.key(KeyEvent {
        key: Key::Text(" ".to_string()),
        mods: Mods::default(),
    });
    runtime.frame(0.0);
    assert!(
        runtime.state.transcript().is_expanded("call-1"),
        "Space on the focused card opens it too: {}",
        words(&runtime)
    );
}
