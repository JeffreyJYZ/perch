//! The dock's grid as geometry: how many lines the box holds, and whether the last of them ends
//! where the box does.
//!
//! This is the measurement that decides what the pty is told, so it is asserted in points rather
//! than in words: the rows a frame reports, the rows the grid paints and the rows the pty hears
//! of are one number, or a strip of the box is painted under the last line.

use perch_term::replay;
use rune::{Env, Runtime};
use serde_json::Value;

use super::super::Shell;
use super::row::row_height;
use super::rows_of;
use crate::shell::view;

/// The window the dock lays its grid out in.
const WINDOW: (f32, f32) = (1280.0, 820.0);

/// A window over one tab showing `bytes` in a 20-column grid of `rows` lines.
fn showing(bytes: &[u8], rows: u16) -> Runtime<Shell> {
    let frame = replay(bytes, 20, rows);
    let mut runtime = Runtime::new(
        Shell::idle(Vec::new(), 0).with_frame("t", frame),
        view,
        WINDOW,
        Env::new(true, 1.0),
    );
    runtime.frame(0.0);
    runtime
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

/// The bounds of the node keyed `key`, in points, as the laid-out tree reports them.
fn bounds(runtime: &Runtime<Shell>, key: &str) -> [f32; 4] {
    let tree = runtime.accessibility_json();
    let node = find(&tree, key).unwrap_or_else(|| panic!("no {key} in the tree"));
    let bounds = &node["bounds"];
    [
        number(&bounds[0]),
        number(&bounds[1]),
        number(&bounds[2]),
        number(&bounds[3]),
    ]
}

/// A JSON number, whatever width it was written at.
fn number(value: &Value) -> f32 {
    value.as_f64().unwrap_or(0.0) as f32
}

#[test]
fn the_grid_paints_a_line_for_every_row_the_frame_reports() {
    let kit = crate::kit::perch(true);
    let line = row_height(&kit);
    // Text on the first and the ninth row: the blank rows between them still hold their lines, so
    // the ninth lands eight lines down instead of under the third.
    let mut runtime = showing(b"one\n\ntwo\n\n\n\n\n\nnine", 9);
    runtime.frame(1.0);

    let frame = runtime.state.active_terminal().unwrap().frame().unwrap();
    let groups = rows_of(frame);
    assert_eq!(
        groups.len(),
        frame.rows.len(),
        "one group for every row the frame reports"
    );
    assert!(groups[1].is_empty(), "a blank row has no runs to draw");
    assert!(!groups[8].is_empty(), "the ninth row carries its text");

    let [_, y, w, h] = bounds(&runtime, "terminal-rows");
    assert_eq!(h, 9.0 * line, "the grid is exactly its nine lines tall");
    assert_eq!(
        usize::from(crate::shell::pump::grid_for(w, h, kit.ty.mono).1),
        frame.rows.len(),
        "the pty is told the rows the frame reports"
    );
    let last = bounds(&runtime, "terminal-row-8");
    assert_eq!(
        last[1],
        y + 8.0 * line,
        "the last line starts eight lines below the top"
    );
    assert_eq!(
        last[1] + last[3],
        y + h,
        "and the grid ends with it: no strip under the last line"
    );
}
