//! What the UI may ask the terminal thread, and the answer channel each
//! request carries.
//!
//! Both enums are the whole protocol. [`Cmd`] is a request with somewhere to
//! reply; [`Inbound`] is that request, a read from the shell, or its end of
//! file — everything the terminal thread will ever see.

use std::sync::mpsc::Sender;

use libghostty_vt::key::{Key, Mods};
use libghostty_vt::terminal::Mode;

/// A key press the terminal should turn into bytes for the shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    /// The physical key.
    pub key: Key,
    /// Modifiers held while it was pressed.
    pub mods: Mods,
}

impl KeyEvent {
    /// A key pressed with no modifiers.
    #[must_use]
    pub fn plain(key: Key) -> Self {
        Self {
            key,
            mods: Mods::empty(),
        }
    }
}

/// What the terminal thread can be told to do.
pub(super) enum Cmd {
    /// Write bytes to the shell.
    Write(Vec<u8>),
    /// Encode a key press against the live terminal and answer with the bytes.
    Key {
        /// The press to encode.
        event: KeyEvent,
        /// Where the encoded bytes go.
        reply: Sender<Vec<u8>>,
    },
    /// Read a mode out of the live terminal and answer whether it is on.
    Mode {
        /// The mode to read.
        mode: Mode,
        /// Where the answer goes.
        reply: Sender<bool>,
    },
    /// Read the grid `offset` rows above the live bottom and answer whether the
    /// viewport moved there.
    ///
    /// The terminal owns the scroll position, so a history view is a request
    /// like a key or a mode read, not a frame the UI can build for itself.
    Scroll {
        /// Rows above the live bottom; `0` is the live bottom.
        offset: usize,
        /// Where the answer goes.
        reply: Sender<bool>,
    },
    /// Resize the PTY and the grid together.
    Resize { cols: u16, rows: u16 },
    /// Stop the thread and let the shell go.
    Shutdown,
}

/// One message into the terminal thread.
pub(super) enum Inbound {
    /// Bytes the shell wrote.
    Output(Vec<u8>),
    /// A request from the UI.
    Cmd(Cmd),
    /// The PTY reader saw end of file.
    Eof,
}
