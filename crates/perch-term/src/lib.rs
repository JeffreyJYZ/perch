//! perch-term: a Ghostty-backed terminal grid on its own thread.
//!
//! `libghostty-vt` handle types are neither `Send` nor `Sync`, so one thread
//! creates, drives and reads a [`Terminal`] for the whole life of a tab; the UI
//! only ever receives a [`Frame`], which is plain owned data. [`replay`] runs
//! the same frame builder over recorded bytes, so tests assert exactly what the
//! painter would see without a PTY in the way.

mod frame;
mod session;

pub use libghostty_vt::key::{Key, Mods};

use libghostty_vt::{Terminal, TerminalOptions};

pub use frame::{CellView, Color, CursorView, Frame, FrameBuilder, MAX_SCROLLBACK, RowView};
pub use session::{KeyEvent, TermError, TermHandle, TermSession};

/// Replay recorded VT bytes at `cols` x `rows` and return the frame the UI
/// would paint.
///
/// This is the live path without a shell: write the bytes into the emulator,
/// then build a frame from its render state. It panics when libghostty cannot
/// build a frame, because in a test that is the failure worth seeing.
#[must_use]
pub fn replay(bytes: &[u8], cols: u16, rows: u16) -> Frame {
    let mut terminal = Terminal::new(TerminalOptions {
        cols,
        rows,
        max_scrollback: MAX_SCROLLBACK,
    })
    .expect("replay: libghostty could not create a terminal");
    let mut builder =
        FrameBuilder::new().expect("replay: libghostty could not create a render state");
    terminal.vt_write(bytes);
    builder
        .build(&terminal)
        .expect("replay: libghostty could not build a frame")
}
