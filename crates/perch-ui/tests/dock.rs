//! The dock's own pointer and keys, driven through a laid-out window: a wheel or the page keys
//! move the terminal's history, and a drag copies the cells it crossed.
//!
//! No pty is involved. A seeded tab draws exactly what [`paint_rows`] makes of a frame, and the
//! tab records the scroll position and the selection the dock asks for, so the same handlers the
//! app runs are the ones a test presses.

use perch_term::replay;
use perch_ui::Shell;
use rune::{Env, Key, KeyEvent, Mods, Runtime, WheelEvent};

/// Wide enough for the dock's rows, tall enough for a grid of a few lines.
const WINDOW: (f32, f32) = (1280.0, 820.0);

/// A laid-out window over one terminal tab showing `bytes` in a `cols` x `rows` grid.
fn window(bytes: &[u8], rows: u16) -> Runtime<Shell> {
    let shell = Shell::idle(Vec::new(), 0).with_frame("t", replay(bytes, 20, rows));
    let mut runtime = Runtime::new(shell, perch_ui::shell::view, WINDOW, Env::new(true, 1.0));
    runtime.frame(0.0);
    runtime
}

/// The bounds of the element keyed `key`, as the tree reports them.
fn bounds(runtime: &Runtime<Shell>, key: &str) -> [f32; 4] {
    let tree = runtime.accessibility_json();
    let node = find(&tree, key).unwrap_or_else(|| panic!("no {key} in {tree}"));
    let bounds = &node["bounds"];
    [
        number(&bounds[0]),
        number(&bounds[1]),
        number(&bounds[2]),
        number(&bounds[3]),
    ]
}

/// The node keyed `key`, depth first.
fn find<'a>(node: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    if node["key"] == key {
        return Some(node);
    }
    let children = node["children"].as_array()?;
    children.iter().find_map(|child| find(child, key))
}

/// A JSON number, whatever width it was written at.
fn number(value: &serde_json::Value) -> f32 {
    value.as_f64().unwrap_or(0.0) as f32
}

/// Everything the tree says, so a test can assert on a word the header shows.
fn words(runtime: &Runtime<Shell>) -> String {
    runtime.accessibility_json().to_string()
}

/// The shifted key a terminal pages its history with.
fn shift(key: Key) -> KeyEvent {
    KeyEvent {
        key,
        mods: Mods {
            shift: true,
            ..Mods::default()
        },
    }
}

#[test]
fn a_page_key_moves_the_grid_a_screen_and_back_to_the_live_bottom() {
    let mut runtime = window(b"one\ntwo\nthree", 8);
    let [x, y, w, h] = bounds(&runtime, "terminal-rows");
    // A click into the grid focuses it, which is what lets the keys reach it.
    runtime.pointer_down(x + w / 2.0, y + h / 2.0);
    runtime.pointer_up(x + w / 2.0, y + h / 2.0);
    runtime.frame(1.0);

    runtime.key(shift(Key::PageUp));
    runtime.frame(1.0);
    assert!(
        words(&runtime).contains("scrolled 7 lines"),
        "a page is one row short of the eight-row grid: {}",
        words(&runtime)
    );

    runtime.key(shift(Key::PageDown));
    runtime.frame(1.0);
    let tree = words(&runtime);
    assert!(
        !tree.contains("scrolled"),
        "the live bottom has nothing to report: {tree}"
    );
    assert!(tree.contains("running"), "and the tab is running: {tree}");
}

#[test]
fn a_page_key_lands_the_header_instead_of_sliding_it() {
    let mut runtime = window(b"one\ntwo\nthree", 8);
    let [x, y, w, h] = bounds(&runtime, "terminal-rows");
    runtime.pointer_down(x + w / 2.0, y + h / 2.0);
    runtime.pointer_up(x + w / 2.0, y + h / 2.0);
    // A second of frames lands the click's own motion, so what follows is the key's.
    runtime.frame(1.0);
    assert!(
        !runtime.frame(1.0 / 60.0),
        "the window is at rest before the key"
    );

    // The page key changes the header's words, which changes their width: the header is the one
    // thing a key moves outside the grid, and a key is direct manipulation, not a transition.
    runtime.key(shift(Key::PageUp));
    let animating = runtime.frame(1.0 / 60.0);
    assert!(
        !animating,
        "a page key lands the header at once: a spring here is the same glide as the caret's"
    );
}

#[test]
fn the_wheel_moves_the_grid_into_the_history() {
    let mut runtime = window(b"one\ntwo", 8);
    let [x, y, w, h] = bounds(&runtime, "terminal-rows");

    runtime.wheel_event(WheelEvent::new(0.0, 100.0).at(x + w / 2.0, y + h / 2.0));
    runtime.frame(1.0);
    let tree = words(&runtime);
    assert!(
        tree.contains("scrolled ") && tree.contains(" lines"),
        "a wheel notch scrolls the terminal's own history: {tree}"
    );

    // The other way returns to the bottom, where nothing is reported any more.
    runtime.wheel_event(WheelEvent::new(0.0, -1000.0).at(x + w / 2.0, y + h / 2.0));
    runtime.frame(1.0);
    assert!(!words(&runtime).contains("scrolled"), "{}", words(&runtime));
}

#[test]
fn a_drag_copies_the_cells_it_crossed_and_a_click_clears_them() {
    let mut runtime = window(b"hello world", 4);
    let [x, y, w, h] = bounds(&runtime, "terminal-rows");
    let cell = w / 20.0;
    let row = y + h / 8.0;

    // From the start of the first row to the middle of "world": eleven columns.
    runtime.pointer_down(x + 1.0, row);
    runtime.pointer_move(x + 10.0 * cell + 1.0, row);
    runtime.pointer_up(x + 10.0 * cell + 1.0, row);
    runtime.frame(1.0);

    let copied = runtime
        .copy_request
        .as_ref()
        .map(|copy| copy.content().text().to_owned());
    assert_eq!(
        copied.as_deref(),
        Some("hello world"),
        "the drag reaches rune's clipboard path"
    );
    let selection = runtime
        .state
        .active_terminal()
        .and_then(perch_ui::TerminalTab::selection)
        .expect("the drag stays highlighted");
    assert_eq!((selection.start_row, selection.start_col), (0, 0));
    assert_eq!((selection.end_row, selection.end_col), (0, 10));

    // A press that does not move is a click: it clears what the last drag selected.
    runtime.pointer_down(x + cell, row);
    runtime.pointer_up(x + cell, row);
    runtime.frame(1.0);
    assert_eq!(
        runtime
            .state
            .active_terminal()
            .and_then(perch_ui::TerminalTab::selection),
        None,
        "a click leaves nothing selected"
    );
}
