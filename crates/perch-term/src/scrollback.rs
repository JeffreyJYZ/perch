//! A terminal plus its viewport pin: the grid can be read above the live bottom.
//!
//! The emulator owns the scroll position, not the painter: `scrollbar()` reports
//! where the viewport sits in the whole scrollable area, and `scroll_viewport`
//! moves it. This type wraps the one [`Terminal`] a session owns with the frame
//! builder that reads it, so a caller can feed bytes, ask for history, and build
//! the frame that would be painted — the same path a replay takes.
//!
//! The pin is deliberately *not* re-applied on every write: the emulator keeps a
//! scrolled viewport where it is while output arrives, which is what stops a
//! view of history from jumping under the user. Scrolling back to offset 0 puts
//! the viewport on the live bottom, and it follows new output from there.

use libghostty_vt::terminal::ScrollViewport;
use libghostty_vt::{Error, Terminal, TerminalOptions};

use crate::frame::{Frame, FrameBuilder, MAX_SCROLLBACK};

/// An emulator and the frame builder that reads it, with a scrollable viewport.
#[derive(Debug)]
pub struct Scrollback<'alloc: 'cb, 'cb> {
    terminal: Terminal<'alloc, 'cb>,
    builder: FrameBuilder<'alloc>,
}

impl<'alloc: 'cb, 'cb> Scrollback<'alloc, 'cb> {
    /// A fresh `cols` x `rows` grid with the crate's scrollback limit.
    pub fn new(cols: u16, rows: u16) -> Result<Self, Error> {
        Ok(Self {
            terminal: Terminal::new(TerminalOptions {
                cols,
                rows,
                max_scrollback: MAX_SCROLLBACK,
            })?,
            builder: FrameBuilder::new()?,
        })
    }

    /// Write VT-encoded bytes to the emulator.
    ///
    /// A scrolled viewport stays where it is: the emulator grows the history
    /// behind it without moving what the user is reading.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.terminal.vt_write(bytes);
    }

    /// Put the viewport `lines` rows above the live bottom.
    ///
    /// `0` is the live bottom. An offset deeper than the history clamps to the
    /// oldest row, which is the emulator's own clamp rather than an error.
    pub fn scroll_to(&mut self, lines: usize) {
        let bottom = self.bottom();
        let row = bottom.saturating_sub(u64::try_from(lines).unwrap_or(u64::MAX));
        self.terminal.scroll_viewport(ScrollViewport::Row(
            usize::try_from(row).unwrap_or(usize::MAX),
        ));
    }

    /// Move the viewport `rows` from where it is: positive toward the live
    /// bottom, negative into history.
    pub fn scroll_by(&mut self, rows: isize) {
        self.terminal.scroll_viewport(ScrollViewport::Delta(rows));
    }

    /// How many rows above the live bottom the viewport currently sits.
    #[must_use]
    pub fn offset(&self) -> usize {
        let bottom = self.bottom();
        let offset = self
            .terminal
            .scrollbar()
            .map_or(bottom, |scrollbar| scrollbar.offset.min(bottom));
        usize::try_from(bottom - offset).unwrap_or(usize::MAX)
    }

    /// Build the frame the painter would draw for the current viewport.
    pub fn frame(&mut self) -> Result<Frame, Error> {
        self.builder.build(&self.terminal)
    }

    /// The emulator, for a read that is not a frame (a key encoding, a mode).
    #[must_use]
    pub fn terminal(&self) -> &Terminal<'alloc, 'cb> {
        &self.terminal
    }

    /// The emulator, for a change a frame cannot express (a resize).
    pub fn terminal_mut(&mut self) -> &mut Terminal<'alloc, 'cb> {
        &mut self.terminal
    }

    /// The row the live bottom viewport starts at: the first row of the last
    /// page of the scrollable area.
    fn bottom(&self) -> u64 {
        self.terminal
            .scrollbar()
            .map_or(0, |scrollbar| scrollbar.total.saturating_sub(scrollbar.len))
    }
}
