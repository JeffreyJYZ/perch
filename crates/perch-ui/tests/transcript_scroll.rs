//! The transcript's own column scroll, driven through a laid-out window: the wheel moves it,
//! growth never yanks a reader who scrolled up, and the bottom follows again once they return.
//! The dock under it keeps its own history and never moves with the column.

use perch_term::replay;
use perch_ui::Shell;
use rune::{Env, Runtime, WheelEvent};
use serde_json::Value;

/// The window the tests lay out, in points: the same size the binary opens.
const WINDOW: (f32, f32) = (1280.0, 820.0);

/// A window over one seeded terminal tab and a transcript of `questions` user turns.
///
/// Forty turns overflow the centre column several times over, so the column has somewhere to
/// scroll to; the dock's three lines are its own history.
fn window(questions: usize) -> Runtime<Shell> {
    let mut shell = Shell::idle(Vec::new(), 0).with_frame("t", replay(b"one\ntwo\nthree", 20, 8));
    for n in 0..questions {
        shell.transcript_mut().push_user(format!("question {n}"));
    }
    let mut runtime = Runtime::new(shell, perch_ui::shell::view, WINDOW, Env::new(true, 1.0));
    runtime.frame(0.0);
    runtime
}

/// The node keyed `key`, depth first.
fn node<'a>(tree: &'a Value, key: &str) -> Option<&'a Value> {
    if tree["key"] == key {
        return Some(tree);
    }
    tree["children"]
        .as_array()?
        .iter()
        .find_map(|child| node(child, key))
}

/// The bounds of the element keyed `key`, as the tree reports them.
fn bounds(runtime: &Runtime<Shell>, key: &str) -> [f32; 4] {
    let tree = runtime.accessibility_json();
    let found = node(&tree, key).unwrap_or_else(|| panic!("no {key} in the tree"));
    let rect = &found["bounds"];
    [
        number(&rect[0]),
        number(&rect[1]),
        number(&rect[2]),
        number(&rect[3]),
    ]
}

/// Where the element keyed `key` was drawn, top and bottom.
fn drawn(runtime: &Runtime<Shell>, key: &str) -> (f32, f32) {
    let rect = runtime
        .elements()
        .into_iter()
        .find(|element| element.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no {key} was drawn"))
        .rect;
    (rect[1], rect[1] + rect[3])
}

/// A JSON number, whatever width it was written at.
fn number(value: &Value) -> f32 {
    value.as_f64().unwrap_or(0.0) as f32
}

/// Where the transcript says it is scrolled to, in its own words, empty at the bottom.
fn header(runtime: &Runtime<Shell>) -> String {
    let tree = runtime.accessibility_json();
    node(&tree, "transcript-scroll-words")
        .and_then(|words| words["name"].as_str())
        .unwrap_or_default()
        .to_owned()
}

/// Everything the dock says, so a test can tell its history from the column's.
fn dock(runtime: &Runtime<Shell>) -> String {
    let tree = runtime.accessibility_json();
    node(&tree, "terminal")
        .map(Value::to_string)
        .unwrap_or_default()
}

#[test]
fn the_wheel_moves_the_transcript_and_the_dock_keeps_its_own_history() {
    let mut runtime = window(40);
    let [x, y, w, h] = bounds(&runtime, "transcript-scroll");

    runtime.wheel_event(WheelEvent::new(0.0, 200.0).at(x + w / 2.0, y + h / 2.0));
    runtime.frame(1.0);
    let up = header(&runtime);
    assert!(
        up.starts_with("scrolled "),
        "the header says how far up the column is: {up:?}"
    );
    assert!(
        up.ends_with(" lines") || up.ends_with(" line"),
        "and says it in lines: {up:?}"
    );
    assert!(
        !dock(&runtime).contains("scrolled"),
        "the dock did not move with the column: {}",
        dock(&runtime)
    );

    // The other way round: the dock's history moves, and the column stays where it was.
    let [dx, dy, dw, dh] = bounds(&runtime, "terminal-rows");
    runtime.wheel_event(WheelEvent::new(0.0, 100.0).at(dx + dw / 2.0, dy + dh / 2.0));
    runtime.frame(1.0);
    assert!(
        dock(&runtime).contains("scrolled"),
        "the dock scrolls its own history: {}",
        dock(&runtime)
    );
    assert_eq!(
        header(&runtime),
        up,
        "the transcript is independent of the dock"
    );
}

#[test]
fn growth_does_not_yank_a_reader_who_scrolled_up_and_the_bottom_follows_again() {
    let mut runtime = window(40);
    let [x, y, w, h] = bounds(&runtime, "transcript-scroll");

    runtime.wheel_event(WheelEvent::new(0.0, 200.0).at(x + w / 2.0, y + h / 2.0));
    runtime.frame(1.0);
    let up = header(&runtime);
    assert!(
        up.contains("scrolled"),
        "the reader is up the column: {up:?}"
    );
    let held = drawn(&runtime, "turn-35");

    // A new turn arrives while the reader is up there: the row they are reading must not move.
    runtime.update(|shell, _| {
        shell.transcript_mut().push_user("a new question");
    });
    runtime.frame(1.0);
    assert_eq!(
        drawn(&runtime, "turn-35"),
        held,
        "new text did not yank the column to the bottom"
    );

    // Back at the bottom, the column follows again: nothing to report, and growth keeps it so.
    runtime.wheel_event(WheelEvent::new(0.0, -5000.0).at(x + w / 2.0, y + h / 2.0));
    runtime.frame(1.0);
    assert_eq!(header(&runtime), "", "the bottom has nothing to report");

    runtime.update(|shell, _| {
        shell.transcript_mut().push_user("and another");
    });
    runtime.frame(1.0);
    assert_eq!(
        header(&runtime),
        "",
        "a following column stays at the bottom as text arrives"
    );
    let [_, top, _, height] = bounds(&runtime, "transcript-scroll");
    let (_, newest) = drawn(&runtime, "turn-41");
    let bottom = top + height;
    assert!(
        newest <= bottom && newest >= bottom - 14.0,
        "the newest turn sits on the viewport's bottom edge, not above it: {newest} in {top}..{bottom}"
    );
}
