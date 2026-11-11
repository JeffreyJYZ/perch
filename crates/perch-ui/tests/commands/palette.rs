//! The palette: ⌘K opens one list over the commands and the app actions, typing filters it, and
//! Enter takes the row.

use perch_ui::Shell;
use rune::{Key, KeyEvent, Mods, Runtime};

use crate::harness::{Fake, window};

/// Clicks the element keyed `key`, from the frame's own tree.
fn click(runtime: &mut Runtime<Shell<Fake>>, key: &str) {
    let tree = runtime.accessibility_json();
    let node = find(&tree, key).unwrap_or_else(|| panic!("no {key} in {tree}"));
    let rect = &node["bounds"];
    let [x, y, w, h] = [0, 1, 2, 3].map(|index| rect[index].as_f64().unwrap_or(0.0) as f32);
    runtime.pointer_down(x + w / 2.0, y + h / 2.0);
    runtime.pointer_up(x + w / 2.0, y + h / 2.0);
    runtime.frame(0.0);
}

fn find<'a>(node: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    if node["key"] == key {
        return Some(node);
    }
    node["children"]
        .as_array()?
        .iter()
        .find_map(|child| find(child, key))
}

fn chord(runtime: &mut Runtime<Shell<Fake>>) {
    runtime.key(KeyEvent {
        key: Key::Command('k'),
        mods: Mods {
            cmd: true,
            ..Mods::default()
        },
    });
    runtime.frame(0.0);
}

fn type_text(runtime: &mut Runtime<Shell<Fake>>, text: &str) {
    for ch in text.chars() {
        runtime.key(KeyEvent {
            key: Key::Text(ch.to_string()),
            mods: Mods::default(),
        });
        runtime.frame(0.0);
    }
}

#[test]
fn command_k_opens_the_palette_over_the_registry_and_the_actions() {
    let dir = tempfile::tempdir().unwrap();
    let mut runtime = window(Fake::one("one", dir.path()));
    chord(&mut runtime);
    assert!(runtime.state.palette_open());
    let rows = runtime.state.palette_rows();
    assert_eq!(
        rows.len(),
        perch_ui::shell::COMMANDS.len() + perch_ui::shell::ACTIONS.len(),
        "every command and every app action"
    );
    assert!(
        runtime
            .accessibility_json()
            .to_string()
            .contains("Command palette")
    );
}

#[test]
fn typing_filters_and_enter_takes_the_app_action() {
    let dir = tempfile::tempdir().unwrap();
    let mut runtime = window(Fake::one("one", dir.path()));
    chord(&mut runtime);
    click(&mut runtime, "palette-query");
    type_text(&mut runtime, "toggle");
    let rows = runtime.state.palette_rows();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].label(), "Toggle dock");

    runtime.key(KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    });
    runtime.frame(0.0);
    assert!(runtime.state.dock_hidden(), "the dock was hidden");
    assert!(!runtime.state.palette_open(), "taking a row closes it");
}

#[test]
fn escape_closes_the_palette_and_a_command_row_runs() {
    let dir = tempfile::tempdir().unwrap();
    let mut runtime = window(Fake::one("one", dir.path()));
    chord(&mut runtime);
    runtime.key(KeyEvent {
        key: Key::Escape,
        mods: Mods::default(),
    });
    runtime.frame(0.0);
    assert!(!runtime.state.palette_open());

    chord(&mut runtime);
    click(&mut runtime, "palette-query");
    type_text(&mut runtime, "new");
    runtime.key(KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    });
    runtime.frame(0.0);
    assert_eq!(
        runtime.state.session_count(),
        2,
        "/new ran from the palette"
    );
}

#[test]
fn a_command_that_wants_an_argument_lands_in_the_composer() {
    let dir = tempfile::tempdir().unwrap();
    let mut runtime = window(Fake::one("one", dir.path()));
    chord(&mut runtime);
    click(&mut runtime, "palette-query");
    type_text(&mut runtime, "rename");
    runtime.key(KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    });
    runtime.frame(0.0);
    assert!(!runtime.state.palette_open());
    assert_eq!(
        runtime.state.prompt_text(),
        "/rename ",
        "the argument is finished where it can be seen"
    );
}
