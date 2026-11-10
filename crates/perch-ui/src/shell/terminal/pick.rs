//! The cells a drag covers, and what a copy takes from them.
//!
//! The pointer arrives in points and the grid is columns, so a drag is mapped cell by cell
//! against the grid element's own size: a terminal draws its frame one row per line and its
//! columns evenly, so the mapping needs no font metrics — the element's width over the frame's
//! column count is one cell.

use perch_term::CellRange;

use super::TerminalTab;
use crate::terminal_view::{Clipboard, copy_selection};

impl TerminalTab {
    /// Starts a selection at one cell: the press corner of a drag. A press clears whatever was
    /// selected, so a click with no drag ends with nothing selected rather than leaving the last
    /// drag highlighted.
    pub(crate) fn press_cell(&mut self, row: u16, col: u16) {
        self.selection = Some(CellRange::new(row, col, row, col));
    }

    /// Moves the drag's free corner to one cell, keeping the pressed corner.
    pub(crate) fn extend_cell(&mut self, row: u16, col: u16) {
        if let Some(range) = self.selection {
            self.selection = Some(CellRange::new(range.start_row, range.start_col, row, col));
        }
    }

    /// The cells the current selection covers, for the painter to highlight.
    ///
    /// It is also what a caller reads to tell a drag from a click: a click clears the range, so
    /// `None` after a release means the pointer never covered more than the cell it pressed.
    pub fn selection(&self) -> Option<CellRange> {
        self.selection
    }

    /// Forgets the selection, as a cancelled drag does.
    pub(crate) fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Ends a drag: copies what it covered and keeps it highlighted so the reader can see what
    /// went to the clipboard. A press that never moved is a click, and a click clears.
    pub(crate) fn release(&mut self, clipboard: &mut impl Clipboard) -> Option<String> {
        let range = self.selection?;
        if range.start_row == range.end_row && range.start_col == range.end_col {
            self.selection = None;
            return None;
        }
        let frame = self.frame.as_ref()?;
        Some(copy_selection(frame, range, clipboard))
    }

    /// Copies the selection again, for ⌘C after a drag: the range stays highlighted, and a copy
    /// of nothing writes nothing.
    pub(crate) fn copy(&mut self, clipboard: &mut impl Clipboard) -> Option<String> {
        let range = self.selection?;
        let frame = self.frame.as_ref()?;
        Some(copy_selection(frame, range, clipboard))
    }
}

/// The cell a pointer at (`x`, `y`) in a `w` x `h` grid names.
///
/// A point outside the grid clamps to its edge rather than naming a row that is not there: a drag
/// that leaves the dock and comes back still selects the last cell it crossed.
pub(crate) fn cell_at(x: f32, y: f32, w: f32, h: f32, cols: u16, rows: u16) -> (u16, u16) {
    (axis(y, h, rows), axis(x, w, cols))
}

/// The cell one axis names: `count` cells over `span` points.
fn axis(at: f32, span: f32, count: u16) -> u16 {
    if span <= 0.0 || count == 0 {
        return 0;
    }
    let cell = span / f32::from(count);
    let index = (at.max(0.0) / cell).floor();
    // `as` after the clamp cannot truncate: the value is under `count`.
    index.min(f32::from(count - 1)) as u16
}

#[cfg(test)]
mod tests {
    use super::{TerminalTab, cell_at};
    use crate::terminal_view::Clipboard;
    use perch_term::{CellRange, replay};

    #[derive(Default)]
    struct Board(Vec<String>);

    impl Clipboard for Board {
        fn copy(&mut self, text: &str) {
            self.0.push(text.to_owned());
        }
    }

    #[test]
    fn a_drag_maps_points_to_the_cells_the_grid_draws() {
        // Ten cells over 80 points, twenty rows over 200: a cell is 8 x 10 points.
        assert_eq!(cell_at(0.0, 0.0, 80.0, 200.0, 10, 20), (0, 0));
        assert_eq!(cell_at(4.0, 9.9, 80.0, 200.0, 10, 20), (0, 0));
        assert_eq!(cell_at(8.0, 10.0, 80.0, 200.0, 10, 20), (1, 1));
        assert_eq!(cell_at(79.9, 199.9, 80.0, 200.0, 10, 20), (19, 9));
        // A pointer that leaves the grid selects its edge, not a row that does not exist.
        assert_eq!(cell_at(-30.0, 900.0, 80.0, 200.0, 10, 20), (19, 0));
        assert_eq!(cell_at(3.0, 3.0, 0.0, 0.0, 0, 0), (0, 0));
    }

    #[test]
    fn a_drag_copies_its_text_and_a_click_clears_it() {
        let frame = replay(b"hello world", 20, 2);
        let mut tab = TerminalTab::new("t", Some(frame));
        let mut board = Board::default();

        tab.press_cell(0, 0);
        tab.extend_cell(0, 10);
        assert_eq!(tab.selection(), Some(CellRange::new(0, 0, 0, 10)));
        assert_eq!(tab.release(&mut board).as_deref(), Some("hello world"));
        assert_eq!(board.0, ["hello world"]);
        assert_eq!(
            tab.selection(),
            Some(CellRange::new(0, 0, 0, 10)),
            "the drag stays highlighted after the copy"
        );

        // ⌘C copies the same range again, without the range moving.
        assert_eq!(tab.copy(&mut board).as_deref(), Some("hello world"));
        assert_eq!(board.0.len(), 2);

        // A press that never moved is a click: it clears, and copies nothing.
        tab.press_cell(1, 4);
        assert_eq!(tab.release(&mut board), None);
        assert_eq!(tab.selection(), None);
        assert_eq!(board.0.len(), 2, "a click writes nothing: {:?}", board.0);

        // A drag over blank cells copies nothing either, and puts no empty string on the board.
        tab.press_cell(1, 0);
        tab.extend_cell(1, 19);
        assert_eq!(tab.release(&mut board).as_deref(), Some(""));
        assert_eq!(board.0.len(), 2, "{:?}", board.0);
    }
}
