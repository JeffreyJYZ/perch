//! The history position: how far the dock is scrolled above the live bottom, and how it says so.
//!
//! The emulator owns the scroll position, so every move is a request to the terminal thread. The
//! tab keeps the resulting offset because the dock's header and the wheel's own step need it in
//! the UI thread, and a request that was not answered must not move it: a stale frame with words
//! claiming another position would be a lie on screen.

use super::TerminalTab;

/// The dock's header says its position with the column's words, so "where am I" reads the same
/// in both: one function in [`crate::transcript`], not a copy per region.
pub(crate) use crate::transcript::scroll_words;

impl TerminalTab {
    /// Rows the grid is scrolled above the live bottom; `0` follows new output.
    pub(crate) fn offset(&self) -> usize {
        self.offset
    }

    /// Puts the viewport `offset` rows above the live bottom and returns whether it moved.
    ///
    /// Offset `0` is the live bottom: from there the emulator follows new output again, which is
    /// what makes "back to the bottom" the same thing as "follow the shell".
    pub(crate) fn scroll_to(&mut self, offset: usize) -> bool {
        let applied = match self.driving() {
            Some(session) => session.io.scroll(offset),
            // A seeded tab has no emulator to move, but the dock still shows where it would be:
            // that is how a test drives the header, and how a tab whose thread is gone keeps the
            // position it was left at.
            None if self.frame.is_some() => true,
            None => return false,
        };
        if applied {
            self.offset = offset;
        }
        applied
    }

    /// Moves the viewport `lines` rows, positive into history, clamped at the live bottom.
    pub(crate) fn scroll_lines(&mut self, lines: isize) -> bool {
        let target = moved(self.offset, lines);
        self.scroll_to(target)
    }

    /// Moves the viewport `pages` pages, positive into history. A page is one row short of the
    /// grid, so the line the reader was on stays in view, as a pager does.
    pub(crate) fn scroll_page(&mut self, pages: isize) -> bool {
        let rows = self.page_rows();
        let lines = pages.saturating_mul(isize::try_from(rows).unwrap_or(isize::MAX));
        self.scroll_lines(lines)
    }

    /// How many rows one page moves: the grid's height, one row short of a full screen.
    fn page_rows(&self) -> usize {
        let rows = usize::from(
            self.frame
                .as_ref()
                .map_or(self.grid.1, |frame| frame.height),
        );
        rows.saturating_sub(1).max(1)
    }

    /// Puts the viewport back on the live bottom, when it is not already there.
    pub(super) fn bottom(&mut self) {
        if self.offset > 0 {
            self.scroll_to(0);
        }
    }
}

/// `offset` moved by `lines`, never below the live bottom.
fn moved(offset: usize, lines: isize) -> usize {
    let moved = isize::try_from(offset)
        .unwrap_or(isize::MAX)
        .saturating_add(lines);
    usize::try_from(moved.max(0)).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::moved;

    #[test]
    fn a_step_that_would_go_past_the_bottom_stops_at_it() {
        assert_eq!(moved(3, -10), 0, "the live bottom is the floor");
        assert_eq!(moved(0, 12), 12);
        assert_eq!(moved(12, -7), 5);
    }
}
