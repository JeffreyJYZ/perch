//! The column laid out through Rune, and the two ways a test reads or presses it.

use perch_ui::Transcript;
use perch_ui::transcript_view;
use rune::{Cx, El, Env, Key, KeyEvent, Mods, Runtime};
use serde_json::Value;

fn view(t: &Transcript, _: &mut Cx<Transcript>) -> El {
    transcript_view(t)
}

/// The column laid out once, unread: for a test that presses a key or the pointer.
pub fn runtime(t: Transcript) -> Runtime<Transcript> {
    let mut runtime = Runtime::new(t, view, (900.0, 700.0), Env::new(true, 2.0));
    runtime.frame(0.0);
    runtime
}

/// The accessibility tree of the column, which carries every word it draws.
pub fn shaped(t: Transcript) -> Value {
    runtime(t).accessibility_json()
}

/// The node keyed `key`, wherever it sits in the tree.
pub fn node<'a>(tree: &'a Value, key: &str) -> Option<&'a Value> {
    if tree["key"] == key {
        return Some(tree);
    }
    tree["children"]
        .as_array()?
        .iter()
        .find_map(|child| node(child, key))
}

/// The description a reader hears for the node keyed `key`.
pub fn described_as(tree: &Value, key: &str) -> Option<String> {
    node(tree, key)
        .and_then(|node| node["description"].as_str())
        .map(str::to_owned)
}

/// ⌘A: the group that was clicked last, else the column's only one.
pub fn select_all(runtime: &mut Runtime<Transcript>) {
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
pub fn click(runtime: &mut Runtime<Transcript>, key: &str) {
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
pub fn rendered(t: Transcript) -> String {
    shaped(t).to_string()
}
