//! Which cells of a row a mark covers, and how many columns a row used.
//!
//! A run's text is one string with no index of its own cells, so a mark at a column is only
//! placeable by walking the row's cells beside it: a blank cell became one space, a wide glyph
//! became its own characters, and the spacer after it became nothing at all.

use perch_term::{CellKind, CellRange, CellView, Frame, RowView};
use rune::Span;

use crate::terminal_view::RowRun;

/// The cell the frame's cursor is drawn at, or `None` when it asks to be hidden.
///
/// A cursor parked outside the grid the frame reports is not drawn either: after a resize the two
/// can disagree for one frame, and a block in a column that is not there would paint over a
/// neighbour.
pub(crate) fn cursor_cell(frame: &Frame) -> Option<(u16, u16)> {
    let cursor = frame.cursor;
    if !cursor.visible || cursor.row >= frame.height || cursor.col >= frame.cols {
        return None;
    }
    Some((cursor.row, cursor.col))
}

/// The columns of `range` that fall on `row`, inclusive, or `None` when the row is outside it.
pub(crate) fn selection_cols(range: CellRange, row: u16, cols: u16) -> Option<(u16, u16)> {
    let (top, left, bottom, right) = ordered(range);
    if cols == 0 || row < top || row > bottom {
        return None;
    }
    let first = if row == top { left } else { 0 };
    let last = if row == bottom { right } else { cols - 1 };
    Some((first.min(cols - 1), last.min(cols - 1)))
}

/// Columns up to and including the last one that draws something.
pub(crate) fn cols_used(row: &RowView) -> u16 {
    u16::try_from(visible_width(row)).unwrap_or(u16::MAX)
}

/// A span over the bytes `run` holds for columns `from` ..= `to`, when it holds any.
///
/// The run's cells are walked from its first column and each contributes what it put in the
/// text — a glyph's own bytes, or the one space a blank cell became — so a range that starts on
/// the spacer of a wide glyph still covers the whole glyph instead of half of it.
pub(crate) fn marked(row: &RowView, run: &RowRun, from: u16, to: u16) -> Option<Span> {
    let total = run.text.len();
    if total == 0 {
        return None;
    }
    let mut offset = 0usize;
    let mut glyph = 0usize;
    let mut start = None;
    let mut end = None;
    for (index, cell) in row.cells.iter().enumerate().skip(usize::from(run.col)) {
        let column = u16::try_from(index).ok()?;
        // The run has ended and the range is behind it: nothing else in this row is its text.
        if offset >= total && column > to {
            break;
        }
        let len = match cell.kind {
            CellKind::Spacer => 0,
            _ => cell.text.len().max(1),
        };
        if len > 0 && offset < total {
            glyph = offset;
        }
        if start.is_none() && column >= from && offset < total {
            start = Some(glyph);
        }
        offset = (offset + len).min(total);
        if column >= to {
            end = Some(offset);
            break;
        }
    }
    let (start, end) = (start?, end?);
    (end > start).then(|| Span::new(start, end))
}

/// The range's corners in top-left to bottom-right order: a drag can run backwards.
fn ordered(range: CellRange) -> (u16, u16, u16, u16) {
    if (range.start_row, range.start_col) <= (range.end_row, range.end_col) {
        (
            range.start_row,
            range.start_col,
            range.end_row,
            range.end_col,
        )
    } else {
        (
            range.end_row,
            range.end_col,
            range.start_row,
            range.start_col,
        )
    }
}

/// Columns up to and including the last one that draws something.
fn visible_width(row: &RowView) -> usize {
    row.cells
        .iter()
        .rposition(|cell| !is_blank(cell))
        .map_or(0, |last| last + 1)
}

/// Whether a cell would draw nothing, so a row's used width can stop before it.
fn is_blank(cell: &CellView) -> bool {
    cell.kind != CellKind::Wide && cell.text.is_empty() && cell.bg == perch_term::Color::Default
}
