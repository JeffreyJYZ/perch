//! The dock as assistive technology reads it, through the accessibility tree: one node per
//! drawn row, the caret and the selection in words, and nothing that is only whitespace.

use perch_term::{CellRange, replay};
use rune::{Env, Runtime};
use serde_json::Value;

use super::super::Shell;
use crate::shell::view;

/// The window the dock lays its grid out in.
const WINDOW: (f32, f32) = (1280.0, 820.0);

/// A laid-out window over `shell`.
fn window(shell: Shell) -> Runtime<Shell> {
    let mut runtime = Runtime::new(shell, view, WINDOW, Env::new(true, 1.0));
    runtime.frame(0.0);
    runtime
}

/// A window over one tab showing `bytes` in a 20-column grid of `rows` lines.
fn showing(bytes: &[u8], rows: u16) -> Runtime<Shell> {
    window(Shell::idle(Vec::new(), 0).with_frame("t", replay(bytes, 20, rows)))
}

/// The node keyed `key`, depth first.
fn find<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    if node["key"] == key {
        return Some(node);
    }
    node["children"]
        .as_array()?
        .iter()
        .find_map(|child| find(child, key))
}

/// Every node of the tree.
fn nodes(tree: &Value) -> Vec<&Value> {
    fn walk<'a>(node: &'a Value, out: &mut Vec<&'a Value>) {
        out.push(node);
        for child in node["children"].as_array().into_iter().flatten() {
            walk(child, out);
        }
    }
    let mut out = Vec::new();
    walk(tree, &mut out);
    out
}

/// The name a reader hears for `node`, empty when it has none.
fn name(node: &Value) -> &str {
    node["name"].as_str().unwrap_or("")
}

/// The key of every node of the dock's grid.
fn rows(tree: &Value) -> Vec<String> {
    nodes(tree)
        .into_iter()
        .filter_map(|node| node["key"].as_str())
        .filter(|key| key.starts_with("terminal-row-"))
        .map(str::to_owned)
        .collect()
}

/// The description the dock put on its grid.
fn grid_words(tree: &Value) -> String {
    find(tree, "terminal-rows").map_or_else(String::new, |grid| {
        grid["description"].as_str().unwrap_or("").to_owned()
    })
}

#[test]
fn a_drawn_row_is_one_node_labelled_with_its_text() {
    // Three style runs, one drawn line: a reader hears the line, not a fragment per colour.
    let tree = showing(b"\x1b[31mred\x1b[0m plain", 4).accessibility_json();
    let row = find(&tree, "terminal-row-0").expect("the row is a node of the tree");
    assert_eq!(row["role"], "row", "a row is a row: {row}");
    assert_eq!(
        name(row),
        "red plain",
        "one label for the whole line: {row}"
    );
    assert!(
        row["children"].as_array().is_none_or(Vec::is_empty),
        "its cells are not nodes of their own: {row}"
    );
}

#[test]
fn a_row_label_is_its_text_with_the_trailing_blanks_trimmed() {
    let tree = showing(b"one\ntwo   \n\nfour", 6).accessibility_json();
    assert_eq!(name(find(&tree, "terminal-row-0").unwrap()), "one");
    let second = name(find(&tree, "terminal-row-1").unwrap());
    assert_eq!(
        second.trim_end(),
        second,
        "no trailing blank is read: {second:?}"
    );
    assert!(second.ends_with("two"), "{second:?}");
    assert!(
        find(&tree, "terminal-row-2").is_none(),
        "a line with nothing on it has nothing to say"
    );
}

#[test]
fn the_caret_is_announced_by_row_and_column_and_says_when_it_is_hidden() {
    let tree = showing(b"hi", 4).accessibility_json();
    assert_eq!(grid_words(&tree), "cursor at row 1, column 3");

    let mut hidden = replay(b"hi", 20, 4);
    hidden.cursor.visible = false;
    let tree = window(Shell::idle(Vec::new(), 0).with_frame("t", hidden)).accessibility_json();
    assert_eq!(grid_words(&tree), "cursor hidden");
}

#[test]
fn a_selection_is_announced_by_range_and_character_count() {
    let mut shell = Shell::idle(Vec::new(), 0).with_frame("t", replay(b"hello world", 20, 4));
    let tab = &mut shell.terminals[0];
    tab.press_cell(0, 0);
    tab.extend_cell(0, 4);
    assert_eq!(
        tab.selection(),
        Some(CellRange::new(0, 0, 0, 4)),
        "a drag over five cells"
    );

    let said = grid_words(&window(shell).accessibility_json());
    assert!(
        said.contains("selection rows 1 to 1, columns 1 to 5"),
        "the range is said in row and column terms: {said:?}"
    );
    assert!(
        said.contains("5 characters"),
        "and how much it holds: {said:?}"
    );
}

#[test]
fn an_exited_shell_says_so_and_its_last_frame_is_not_announced() {
    let mut shell = Shell::idle(Vec::new(), 0).with_frame("t", replay(b"last words", 20, 4));
    shell.terminals[0].stop_for_test("the shell exited");
    let tree = window(shell).accessibility_json();
    let dock = find(&tree, "terminal").expect("the dock");
    let said = dock.to_string();
    assert!(said.contains("exited"), "the header says the state: {said}");
    assert!(said.contains("the shell exited"), "and why: {said}");
    assert!(
        !said.contains("live-"),
        "a frozen frame is not announced as it changes: {said}"
    );
    assert!(
        find(&tree, "terminal-row-0").is_none(),
        "the last frame is not painted: {said}"
    );
}

#[test]
fn nothing_is_named_with_whitespace_alone_and_a_repaint_adds_no_row() {
    let mut runtime = showing(b"one\n\n  \nfour", 6);
    let first = rows(&runtime.accessibility_json());
    assert_eq!(first.len(), 2, "two rows carry text: {first:?}");

    runtime.frame(1.0);
    let tree = runtime.accessibility_json();
    let again = rows(&tree);
    assert_eq!(again, first, "a repaint keeps the same row nodes");
    let mut unique = again.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        again.len(),
        "no row is drawn twice: {again:?}"
    );

    for node in nodes(&tree) {
        let label = name(node);
        assert!(
            label.is_empty() || !label.trim().is_empty(),
            "a name of whitespace alone says nothing: {node}"
        );
    }
}
