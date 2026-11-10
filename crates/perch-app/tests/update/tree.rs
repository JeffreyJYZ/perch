//! The offer as the window draws it: one line in the column, and the button a press reaches.

use perch_ui::Shell;
use perch_ui::shell::{Live, Panels};
use rune::{Env, Runtime};
use serde_json::Value;

use crate::harness::{Fake, HELLO, MANIFEST, app, data, line, manifest, pump_until};

/// Lays `shell` out at the binary's window size and returns its accessibility tree.
fn tree_of<L>(shell: Shell<L>) -> Value
where
    L: Live + Panels + 'static,
{
    let mut runtime = Runtime::new(
        shell,
        perch_ui::shell::view,
        (1280.0, 820.0),
        Env::new(true, 2.0),
    );
    runtime.frame(0.0);
    runtime.accessibility_json()
}

/// The node the app keyed `key`, wherever it sits.
fn find_key<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    if node.get("key").and_then(Value::as_str) == Some(key) {
        return Some(node);
    }
    let children = node.get("children").and_then(Value::as_array)?;
    children.iter().find_map(|child| find_key(child, key))
}

/// Whether anything in `node`'s subtree says `word` out loud.
fn says(node: &Value, word: &str) -> bool {
    for field in ["name", "value", "placeholder", "description"] {
        if node
            .get(field)
            .and_then(Value::as_str)
            .is_some_and(|text| text.contains(word))
        {
            return true;
        }
    }
    node.get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|child| says(child, word))
}

#[test]
fn an_offer_is_one_line_in_the_window_with_a_button_of_its_own() {
    let fake = Fake::new(&[(MANIFEST, &manifest("99.0.0", HELLO))]);
    let dir = data(Some(MANIFEST));
    let mut app = app(&dir, fake);
    assert!(pump_until(&mut app, |app| line(app)
        .is_some_and(|words| words.contains("available"))));
    let sessions = app.metas();

    let tree = tree_of(Shell::new(sessions, 0).with_live(app));

    let column = find_key(&tree, "plugins").expect("the column is drawn");
    assert!(
        says(column, "update 99.0.0 available: Fixed the updater."),
        "the offer is painted in the column: {column}"
    );
    let button = find_key(&tree, "panel-button-stage")
        .expect("the offer declares its download control in the tree");
    assert!(says(button, "Download"), "the button says what it does");
}
