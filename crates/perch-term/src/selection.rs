//! Turning a cell range into the text a copy puts on the clipboard.
//!
//! The grid is columns, so a copy cannot be a slice of the painted runs: a run exists only
//! where a *style* changes, and a wide glyph covers two columns while a spacer after it covers
//! one and holds no text. This module walks the cells a drag covered, row by row, and writes
//! what the user sees.

use crate::frame::{CellKind, Frame, RowView};

/// A range of grid cells, inclusive at both ends, in viewport coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellRange {
    /// Row the selection starts on, 0-based from the top of the viewport.
    pub start_row: u16,
    /// Column the selection starts at, 0-based from the left.
    pub start_col: u16,
    /// Row the selection ends on, inclusive.
    pub end_row: u16,
    /// Column the selection ends at, inclusive.
    pub end_col: u16,
}

impl CellRange {
    /// A range from one cell to another, in the order the drag happened.
    #[must_use]
    pub const fn new(start_row: u16, start_col: u16, end_row: u16, end_col: u16) -> Self {
        Self {
            start_row,
            start_col,
            end_row,
            end_col,
        }
    }
}

/// The text `range` covers: what a copy of that drag should put on the clipboard.
///
/// Three rules make the result read like the screen. Trailing blanks are trimmed from every
/// row, because the padding on the right of a line is a column the shell never wrote. A wide
/// glyph is copied whole even when the range starts on the second column it covers, because
/// half a glyph is not a character. A row that soft-wrapped continues the line above it, so no
/// newline goes between the two; only a row that really ended gets one.
///
/// A range that outlives a resize, and so names rows or columns the frame no longer has, stops
/// at the grid instead of panicking: the selection was made against an older frame.
#[must_use]
pub fn selection_text(frame: &Frame, range: CellRange) -> String {
    let ((start_row, start_col), (end_row, end_col)) = ordered(range);
    if frame.rows.is_empty() {
        return String::new();
    }
    // The grid wins over the range: rows past the last one are not there to copy.
    let end_row = end_row.min(u16::try_from(frame.rows.len() - 1).unwrap_or(u16::MAX));
    if start_row > end_row {
        return String::new();
    }
    let mut out = String::new();
    for (index, row) in frame.rows.iter().enumerate() {
        let Ok(row_index) = u16::try_from(index) else {
            break;
        };
        if row_index < start_row {
            continue;
        }
        if row_index > end_row {
            break;
        }
        let first = if row_index == start_row { start_col } else { 0 };
        let last = (row_index == end_row).then_some(end_col);
        out.push_str(&row_text(row, first, last));
        if row_index != end_row && !row.wrapped {
            out.push('\n');
        }
    }
    // A drag through the blank rows under the last line copies those lines as nothing, not as
    // one newline each, so the trailing breaks go.
    let kept = out.trim_end_matches('\n').len();
    out.truncate(kept);
    out
}

/// The range's two corners in top-left to bottom-right order.
///
/// A drag can run backwards, and a selection made by double-click and then extended can have
/// either corner first, so the mapping never assumes the order it was given.
fn ordered(range: CellRange) -> ((u16, u16), (u16, u16)) {
    let start = (range.start_row, range.start_col);
    let end = (range.end_row, range.end_col);
    if start <= end {
        (start, end)
    } else {
        (end, start)
    }
}

/// One row's text from `first_col` up to and including `last_col`, trailing blanks trimmed.
///
/// `last_col` is `None` for a row the range passes through, which runs to the row's end.
fn row_text(row: &RowView, first_col: u16, last_col: Option<u16>) -> String {
    let mut first = usize::from(first_col);
    // A selection that starts on the spacer half of a wide glyph still means the glyph.
    if row
        .cells
        .get(first)
        .is_some_and(|cell| cell.kind == CellKind::Spacer)
    {
        first = first.saturating_sub(1);
    }
    let end = last_col.map_or(row.cells.len(), |col| {
        (usize::from(col) + 1).min(row.cells.len())
    });
    let mut text = String::new();
    for cell in row.cells.get(first..end).unwrap_or_default() {
        match cell.kind {
            // The spacer draws nothing; the wide glyph before it already carried both columns.
            CellKind::Spacer => {}
            CellKind::Narrow if cell.text.is_empty() => text.push(' '),
            CellKind::Narrow | CellKind::Wide => text.push_str(&cell.text),
        }
    }
    let trimmed = text.trim_end_matches(' ').len();
    text.truncate(trimmed);
    text
}
