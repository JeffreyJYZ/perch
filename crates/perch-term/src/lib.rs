//! perch-term: a Ghostty-backed terminal grid on its own thread.
//!
//! `libghostty-vt` handle types are neither `Send` nor `Sync`, so one thread
//! creates, drives and reads a [`Terminal`] for the whole life of a tab; the UI
//! only ever receives a [`Frame`], which is plain owned data. [`replay`] runs
//! the same frame builder over recorded bytes, so tests assert exactly what the
//! painter would see without a PTY in the way. [`replay_at`] runs it at a scroll
//! offset, which is how a history view is read without a live terminal.

mod frame;
mod keys;
mod paste;
mod scrollback;
mod selection;
mod session;

pub use libghostty_vt::key::{Key, Mods};
pub use libghostty_vt::screen::Screen;
pub use libghostty_vt::terminal::{Mode, ModeKind};

use std::time::{Duration, Instant};

pub use libghostty_vt::{Terminal, TerminalOptions};

pub use frame::{
    CellKind, CellView, Color, CursorView, Frame, FrameBuilder, MAX_SCROLLBACK, RowView,
};
pub use keys::{encode_key, encode_key_for_terminal};
pub use paste::{PASTE_END, PASTE_START, encode_paste};
pub use scrollback::Scrollback;
pub use selection::{CellRange, selection_text};
pub use session::{KeyEvent, TermError, TermHandle, TermSession};

/// How a replay's time was spent.
///
/// The split matters because the two stages have different owners:
/// `libghostty-vt-sys` builds the vendored Ghostty core with Zig, so in a cargo
/// dev profile it runs in Zig's `Debug` mode and is an order of magnitude
/// slower than the same core in a release profile. A budget on this crate's
/// frame builder must not be measured against that, and a budget on the whole
/// replay must say which profile it applies to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stages {
    /// Feeding the recorded bytes to the emulator.
    pub feed: Duration,
    /// Building the frame from the emulator's render state.
    pub build: Duration,
}

impl Stages {
    /// Parsing plus frame building.
    #[must_use]
    pub fn total(self) -> Duration {
        self.feed + self.build
    }
}

/// Replay recorded VT bytes at `cols` x `rows` and return the frame the UI
/// would paint.
///
/// This is the live path without a shell: write the bytes into the emulator,
/// then build a frame from its render state. It panics when libghostty cannot
/// build a frame, because in a test that is the failure worth seeing.
#[must_use]
pub fn replay(bytes: &[u8], cols: u16, rows: u16) -> Frame {
    replay_stages(bytes, cols, rows).0
}

/// [`replay`], with the viewport `offset` rows above the live bottom.
///
/// This is the history view a caller gets without a live terminal: the same
/// recorded bytes, read through the same frame builder, only scrolled. `0` is
/// the live bottom and is identical to [`replay`]; an offset deeper than the
/// history clamps to the oldest row.
///
/// It is not a method on [`Frame`] because a frame holds one viewport's worth of
/// cells: the rows above it are not in it. The history lives in the emulator, so
/// this answers from a [`Scrollback`] instead of from a frame.
#[must_use]
pub fn replay_at(bytes: &[u8], cols: u16, rows: u16, offset: usize) -> Frame {
    let mut scrollback =
        Scrollback::new(cols, rows).expect("replay_at: libghostty could not create a terminal");
    scrollback.feed(bytes);
    scrollback.scroll_to(offset);
    scrollback
        .frame()
        .expect("replay_at: libghostty could not build a frame")
}

/// [`replay`], with the time each stage took.
#[must_use]
pub fn replay_stages(bytes: &[u8], cols: u16, rows: u16) -> (Frame, Stages) {
    let started = Instant::now();
    let mut terminal = Terminal::new(TerminalOptions {
        cols,
        rows,
        max_scrollback: MAX_SCROLLBACK,
    })
    .expect("replay: libghostty could not create a terminal");
    terminal.vt_write(bytes);
    let feed = started.elapsed();

    let build_started = Instant::now();
    let mut builder =
        FrameBuilder::new().expect("replay: libghostty could not create a render state");
    let frame = builder
        .build(&terminal)
        .expect("replay: libghostty could not build a frame");
    let build = build_started.elapsed();

    (frame, Stages { feed, build })
}
