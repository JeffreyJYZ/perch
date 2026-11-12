//! A tool card names its target: the path a read opened, the command a bash call ran, and nothing
//! when the arguments name neither.

use perch_agent::AgentEvent;
use perch_ui::{Item, Transcript};

use crate::render::rendered;
use crate::support::call;

/// The card `t` holds for `id`, as its fields.
fn card<'a>(t: &'a Transcript, id: &str) -> &'a Item {
    t.items()
        .iter()
        .find(|item| matches!(item, Item::Tool { id: card, .. } if card == id))
        .unwrap_or_else(|| panic!("no card {id}: {:?}", t.items()))
}

/// The target a call with `args` carries.
fn target_of(args: serde_json::Value) -> Option<String> {
    let mut t = Transcript::default();
    t.apply(&AgentEvent::ToolCall(perch_agent::ToolCall {
        id: "c".into(),
        name: "tool".into(),
        args,
    }));
    match card(&t, "c") {
        Item::Tool { target, .. } => target.clone(),
        other => panic!("a call is a card, got {other:?}"),
    }
}

#[test]
fn a_read_names_the_path_it_opens() {
    assert_eq!(
        target_of(serde_json::json!({ "path": "src/main.rs" })),
        Some("src/main.rs".to_string())
    );
    // The card header carries it too, not only the item.
    let mut t = Transcript::default();
    t.apply(&AgentEvent::ToolCall(call("a", "read")));
    assert!(
        rendered(t).contains("src/main.rs"),
        "the card draws the path beside the tool name"
    );
}

#[test]
fn a_bash_call_names_its_command() {
    assert_eq!(
        target_of(serde_json::json!({ "command": "cargo test" })),
        Some("cargo test".to_string())
    );
}

#[test]
fn a_call_with_no_target_argument_has_none() {
    assert_eq!(
        target_of(serde_json::json!({})),
        None,
        "no argument, no target"
    );
    assert_eq!(
        target_of(serde_json::json!({ "path": "" })),
        None,
        "an empty argument names nothing, so it is not drawn as an empty run of mono"
    );
    assert_eq!(
        target_of(serde_json::json!({ "path": "   " })),
        None,
        "whitespace is not a target either"
    );
}

#[test]
fn the_first_naming_argument_wins_when_a_call_has_several() {
    assert_eq!(
        target_of(serde_json::json!({ "pattern": "fn main", "path": "src/main.rs" })),
        Some("src/main.rs".to_string()),
        "the card names one thing: the path a call was given, before the pattern"
    );
}
