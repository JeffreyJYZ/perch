//! Key encoding, resize plumbing and the flood budget.
//!
//! The paint test lives in `perch-ui`, because run-length grouping is the
//! painter's job; these prove the terminal side of the same task.

use std::{
    path::Path,
    time::{Duration, Instant},
};

use perch_term::{
    Frame, FrameBuilder, Key, KeyEvent, Mods, Screen, TermSession, Terminal, TerminalOptions,
    encode_key, encode_key_for_terminal, replay_stages,
};

/// The frame budget the plan states, for the build that ships.
const BUDGET: Duration = Duration::from_millis(150);

/// The same budget for a cargo dev profile.
///
/// `libghostty-vt-sys` builds the vendored Ghostty core with Zig and picks
/// Zig's `Debug` mode whenever cargo is in the dev profile, which is roughly a
/// thousand times slower than `ReleaseFast` on the same bytes. The measured
/// dev-profile replay is about 10.5 s, so this ceiling is not the acceptance
/// number — it exists to fail a real regression (a quadratic parse, a leak)
/// while the release run asserts the 150 ms the plan asks for.
const DEV_BUDGET: Duration = Duration::from_secs(15);

#[test]
fn control_and_arrow_keys_are_encoded() {
    // The plan spells these `Key::Char('c')` and `Mods::NONE`; the library has
    // a physical-key enum and bitflags instead, so `C` plus CTRL is the same
    // press expressed in the types that exist.
    assert_eq!(
        encode_key(KeyEvent {
            key: Key::C,
            mods: Mods::CTRL,
        }),
        b"\x03".to_vec()
    );
    assert_eq!(
        encode_key(KeyEvent::plain(Key::ArrowUp)),
        b"\x1b[A".to_vec()
    );
    assert_eq!(encode_key(KeyEvent::plain(Key::Enter)), b"\r".to_vec());
}

#[test]
fn modifier_and_function_keys_are_encoded() {
    let cases = [
        (Key::ArrowUp, Mods::CTRL, b"\x1b[1;5A".to_vec()),
        (Key::B, Mods::ALT, b"\x1bb".to_vec()),
        (Key::Tab, Mods::SHIFT, b"\x1b[Z".to_vec()),
        (Key::F5, Mods::empty(), b"\x1b[15~".to_vec()),
        (Key::A, Mods::empty(), b"a".to_vec()),
        (Key::A, Mods::SHIFT, b"A".to_vec()),
    ];
    for (key, mods, expected) in cases {
        let encoded = encode_key(KeyEvent { key, mods });
        eprintln!("encode {key:?} {mods:?} -> {encoded:?}");
        assert_eq!(encoded, expected, "{key:?} with {mods:?}");
    }
}

#[test]
fn application_cursor_mode_changes_the_arrow_encoding() {
    let mut terminal = Terminal::new(TerminalOptions {
        cols: 80,
        rows: 24,
        max_scrollback: 0,
    })
    .expect("create a terminal");
    // DECCKM, which a full-screen program turns on and which only the terminal
    // knows about: without reading it back, `Up` is the wrong three bytes.
    terminal.vt_write(b"\x1b[?1h");

    let encoded = encode_key_for_terminal(KeyEvent::plain(Key::ArrowUp), &terminal);
    eprintln!("encode Up with DECCKM -> {encoded:?}");
    assert_eq!(encoded, b"\x1bOA".to_vec());
}

#[test]
fn flood_keeps_frame_build_under_budget() {
    let path = "tests/fixtures/flood.bin";
    let bytes = std::fs::read(path).unwrap_or_else(|error| {
        panic!(
            "read {path}: {error}; generate it with `cargo run -p perch-term --example make_flood`"
        )
    });
    let total_budget = if cfg!(debug_assertions) {
        DEV_BUDGET
    } else {
        BUDGET
    };

    let start = Instant::now();
    let (frame, stages) = replay_stages(&bytes, 120, 40);
    let elapsed = start.elapsed();

    eprintln!(
        "flood: {} bytes ({} KiB) replayed into a {}x{} frame: feed {:?}, build {:?}, \
         total {:?}; budget {:?} (frame build {:?})",
        bytes.len(),
        bytes.len() / 1024,
        frame.cols,
        frame.height,
        stages.feed,
        stages.build,
        elapsed,
        total_budget,
        BUDGET
    );

    assert_eq!(frame.rows.len(), 40, "one row per viewport line");
    assert_eq!(frame.rows[0].cells.len(), 120, "one cell per column");
    assert!(
        stages.build < BUDGET,
        "frame build took {:?}, budget {BUDGET:?}",
        stages.build
    );
    assert!(
        elapsed < total_budget,
        "flood took {elapsed:?}, budget {total_budget:?}"
    );
}

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
