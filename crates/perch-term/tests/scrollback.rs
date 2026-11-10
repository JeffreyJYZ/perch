//! Reading the grid above the live bottom, without a PTY in the way.
//!
//! The live terminal keeps its scrollback pin itself: a write while scrolled does
//! not move the viewport. These tests pin that behaviour through [`Scrollback`],
//! and pin that a stateless replay can ask for any offset and get the rows a real
//! terminal would have shown. The live channel has its own file, `live_scroll.rs`.

mod support;

use perch_term::{Frame, Scrollback, replay, replay_at};
use support::line;

/// The offset the history tests scroll by.
const UP: usize = 10;

#[test]
fn scrolling_up_shows_the_rows_above_the_live_ones() {
    let bytes = numbered(200);
    let live = replay(&bytes, 20, 5);
    let scrolled = replay_at(&bytes, 20, 5, UP);

    // 200 numbered lines into a 5-row grid leaves the last screen at 196..200.
    assert_eq!(line(&live, 0), "line 196");
    assert_eq!(line(&live, 4), "");

    // Ten rows up is the same window, shifted: 186..190.
    assert_eq!(scrolled.rows.len(), 5);
    assert_eq!(line(&scrolled, 0), "line 186");
    assert_eq!(line(&scrolled, 4), "line 190");
}

#[test]
fn offset_zero_equals_the_live_frame() {
    let bytes = numbered(200);
    assert_eq!(replay_at(&bytes, 20, 5, 0), replay(&bytes, 20, 5));
}

#[test]
fn an_offset_past_the_history_clamps_to_the_top() {
    let bytes = numbered(30);
    let frame = replay_at(&bytes, 20, 5, 10_000);

    assert_eq!(
        frame.rows.len(),
        5,
        "a clamped scroll still answers a whole frame"
    );
    assert_eq!(
        line(&frame, 0),
        "line 0",
        "the clamp lands on the oldest row"
    );
    assert_eq!(line(&frame, 4), "line 4");
}

#[test]
fn output_while_scrolled_leaves_the_view_alone() {
    let mut scrollback = Scrollback::new(20, 5).expect("create a scrollback");
    scrollback.feed(&numbered(200));
    scrollback.scroll_to(UP);

    let before = rows(&mut scrollback);
    assert_eq!(before[0], "line 186");

    scrollback.feed(&numbered_from(200, 5));
    assert_eq!(
        rows(&mut scrollback),
        before,
        "new output must not move a scrolled view"
    );

    // Back to offset 0 follows the live bottom again, which the new output moved.
    scrollback.scroll_to(0);
    let live = rows(&mut scrollback);
    assert_ne!(live, before);
    assert!(
        live.iter().any(|row| row.starts_with("line 20")),
        "the live bottom shows the newest output: {live:?}"
    );
}

#[test]
fn scrolling_by_a_delta_is_relative_to_the_current_view() {
    let mut scrollback = Scrollback::new(20, 5).expect("create a scrollback");
    scrollback.feed(&numbered(200));

    scrollback.scroll_to(UP);
    assert_eq!(scrollback.offset(), UP);

    // Positive moves toward the live bottom, so five rows down from ten is five.
    scrollback.scroll_by(5);
    assert_eq!(scrollback.offset(), 5);
    assert_eq!(
        line(&scrollback.frame().expect("build a frame"), 0),
        "line 191"
    );

    scrollback.scroll_by(-5);
    assert_eq!(scrollback.offset(), UP);
}

/// `count` numbered lines, each ending in a carriage return and newline.
fn numbered(count: usize) -> Vec<u8> {
    numbered_from(0, count)
}

/// `count` numbered lines starting at `start`.
fn numbered_from(start: usize, count: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    for index in start..start + count {
        bytes.extend_from_slice(format!("line {index}\r\n").as_bytes());
    }
    bytes
}

/// Every row of the frame the scrollback would paint right now.
fn rows(scrollback: &mut Scrollback<'_, '_>) -> Vec<String> {
    let frame: Frame = scrollback.frame().expect("build a frame");
    (0..frame.rows.len()).map(|row| line(&frame, row)).collect()
}
