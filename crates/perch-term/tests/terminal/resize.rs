//! A resize: the pty is told first, then the grid, and a frame is built after both.

use std::path::Path;
use std::time::{Duration, Instant};

use perch_term::{Frame, FrameBuilder, Screen, TermSession, Terminal, TerminalOptions};

#[test]
fn resize_reaches_the_pty_then_the_grid_and_then_a_frame() {
    let (handle, frames) = TermSession::spawn(Path::new("/"), 40, 12).expect("spawn a shell");
    let first = next_frame(&frames, None);
    assert_eq!((first.cols, first.height), (40, 12));
    assert_eq!(first.rows.len(), 12);

    // A resize must be answered by a frame that is already the new size: the
    // PTY is told first, the grid second, and the frame is built after both.
    // A frame published from the old grid would still have 40 columns.
    handle.resize(70, 20);

    let resized = next_frame(
        &frames,
        Some(|frame: &Frame| frame.cols == 70 && frame.height == 20),
    );
    assert_eq!((resized.cols, resized.height), (70, 20));
    assert_eq!(resized.rows.len(), 20);
    assert!(
        resized.rows.iter().all(|row| row.cells.len() == 70),
        "every row of the resized grid is 70 cells wide"
    );
}

#[test]
fn a_resize_leaves_the_alternate_screen_whole() {
    let mut terminal = Terminal::new(TerminalOptions {
        cols: 40,
        rows: 10,
        max_scrollback: 0,
    })
    .expect("create a terminal");
    let mut builder = FrameBuilder::new().expect("create a render state");

    // A full-screen program: alternate screen on (DECSET 1049), one known line, then the
    // cursor parked a few rows below it.
    terminal.vt_write(b"\x1b[?1049h\x1b[3;5Hhello\x1b[7;9H");
    let before = builder.build(&terminal).expect("frame before the resize");
    assert_eq!((before.cols, before.height), (40, 10));
    assert_eq!(line(&before, 2), "hello");
    assert_eq!((before.cursor.row, before.cursor.col), (6, 8));

    terminal.resize(72, 20, 0, 0).expect("resize the grid");

    let after = builder.build(&terminal).expect("frame after the resize");
    assert_eq!((after.cols, after.height), (72, 20));
    assert_eq!(
        terminal.active_screen().expect("active screen"),
        Screen::Alternate,
        "a resize must not fall back to the primary screen"
    );
    assert_eq!(
        line(&after, 2),
        "hello",
        "the program's line stays at the row and column it wrote"
    );
    assert_eq!(
        (after.cursor.row, after.cursor.col),
        (6, 8),
        "the cursor stays where the program left it"
    );
}

/// One row's text, which is every glyph on it: blanks carry an empty string.
fn line(frame: &Frame, row: usize) -> String {
    frame.rows[row]
        .cells
        .iter()
        .map(|cell| cell.text.as_str())
        .collect()
}

/// Read frames until `accept` says yes, or fail after a bounded wait.
fn next_frame(
    frames: &std::sync::mpsc::Receiver<Frame>,
    accept: Option<fn(&Frame) -> bool>,
) -> Frame {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let frame = frames
            .recv_timeout(left)
            .expect("a frame from the terminal thread before the deadline");
        if accept.is_none_or(|accept| accept(&frame)) {
            return frame;
        }
    }
}
