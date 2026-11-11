//! Settings as a tab: one strip carries the sessions, the opened files and the settings, and its
//! one active tab is the pane the window is showing.

use std::path::PathBuf;

use perch_store::SessionMeta;
use perch_ui::Shell;
use rune::Runtime;
use serde_json::Value;

use crate::fake::Fake;
use crate::harness::{click, layout};

/// A session with `id` as its title, so a test names the tab it presses.
fn session(id: &str) -> SessionMeta {
    SessionMeta {
        id: id.to_string(),
        title: id.to_string(),
        worktree: PathBuf::from("/work"),
        branch: "dev".to_string(),
        model: String::new(),
        created: 0,
    }
}

/// The node keyed `key`, wherever it sits in the tree.
fn find<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    if node["key"] == key {
        return Some(node);
    }
    node["children"]
        .as_array()?
        .iter()
        .find_map(|child| find(child, key))
}

/// Every tab drawn with `state`, so "the active state must be one" can be counted.
fn tabs_in(runtime: &Runtime<Shell<Fake>>, state: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![runtime.accessibility_json()];
    while let Some(node) = stack.pop() {
        let is_tab = node["role"] == "tab"
            && node["states"]
                .as_array()
                .is_some_and(|states| states.iter().any(|word| word.as_str() == Some(state)));
        if is_tab && let Some(key) = node["key"].as_str() {
            found.push(key.to_string());
        }
        if let Some(children) = node["children"].as_array() {
            stack.extend(children.iter().cloned());
        }
    }
    found.sort();
    found
}

#[test]
fn the_settings_tab_opens_the_pane_and_a_session_tab_returns_to_the_session() {
    let shell = Shell::new(vec![session("one"), session("two")], 0).with_live(Fake::default());
    let mut runtime = layout(shell);

    // The strip carries the settings after the sessions, with its own glyph.
    let tree = runtime.accessibility_json();
    let settings = find(&tree, "t-settings").expect("the strip draws a settings tab");
    assert_eq!(settings["role"], "tab");
    assert_eq!(settings["name"], "Settings");
    assert!(
        settings["position"].as_u64().unwrap_or(0) > 2,
        "the settings tab comes after both sessions: {}",
        settings["position"]
    );
    assert_eq!(
        tabs_in(&runtime, "selected"),
        vec!["t-one".to_string()],
        "the first session is the active tab"
    );

    // Selecting it opens the pane, and it is the tab shown as active.
    click(&mut runtime, "t-settings");
    assert!(runtime.state.settings_open(), "the tab opened the settings");
    assert_eq!(
        tabs_in(&runtime, "selected"),
        vec!["t-settings".to_string()]
    );

    // Selecting a session leaves the pane and shows that session.
    click(&mut runtime, "t-two");
    assert!(
        !runtime.state.settings_open(),
        "a session tab leaves the settings"
    );
    assert_eq!(runtime.state.active(), 1, "the pressed session is shown");
    assert_eq!(tabs_in(&runtime, "selected"), vec!["t-two".to_string()]);
}

#[test]
fn the_settings_pane_is_drawn_once_and_its_active_state_is_one() {
    let shell = Shell::new(vec![session("one")], 0).with_live(Fake::default());
    let mut runtime = layout(shell);
    click(&mut runtime, "t-settings");
    assert!(runtime.state.settings_open());

    let panes = runtime
        .elements()
        .into_iter()
        .filter(|element| element.key.as_deref() == Some("settings"))
        .count();
    assert!(
        panes == 1,
        "the pane the tab selects is drawn once: {panes}"
    );
    assert_eq!(
        tabs_in(&runtime, "selected"),
        vec!["t-settings".to_string()],
        "one tab is the active one, and it is the settings tab"
    );
}
