//! The dock in words: a row's text, where the caret is, and what a selection covers.
//!
//! A screen reader cannot see a block cursor or a wash of colour, so the dock says the caret and
//! the selection the same way it says a row's text: a row's label and the grid's description.

use perch_term::{CellKind, CellRange, Frame, RowView, selection_text};

/// One drawn row as a reader hears it: every cell's text, with the trailing blanks trimmed.
///
/// A blank cell inside the line is a space because it holds a column the reader must cross; the
/// blanks after the last glyph are not text at all.
pub(super) fn line(row: &RowView) -> String {
    let mut out = String::new();
    for cell in &row.cells {
        match cell.kind {
            CellKind::Spacer => {}
            _ if cell.text.is_empty() => out.push(' '),
            _ => out.push_str(&cell.text),
        }
    }
    let end = out.trim_end().len();
    out.truncate(end);
    out
}

/// Where the caret is, in words, counting from one the way a reader counts.
pub(super) fn cursor(frame: &Frame) -> String {
    match frame.cursor.visible {
        true => format!(
            "cursor at row {}, column {}",
            one_based(frame.cursor.row),
            one_based(frame.cursor.col)
        ),
        false => "cursor hidden".to_owned(),
    }
}

/// What the selection covers: its corners in row and column terms, and how much text it holds.
pub(super) fn selection(frame: &Frame, range: CellRange) -> String {
    let ((top, left), (bottom, right)) = corners(range);
    format!(
        "selection rows {} to {}, columns {} to {}, {} characters",
        one_based(top),
        one_based(bottom),
        one_based(left),
        one_based(right),
        selection_text(frame, range).chars().count()
    )
}

/// What the dock says about the grid itself: the caret, and the selection when there is one.
pub(super) fn grid(frame: &Frame, selected: Option<CellRange>) -> String {
    let mut said = cursor(frame);
    if let Some(range) = selected {
        said.push_str("; ");
        said.push_str(&selection(frame, range));
    }
    said
}

/// The range's corners in reading order: a drag can run backwards.
fn corners(range: CellRange) -> ((u16, u16), (u16, u16)) {
    let (start, end) = (
        (range.start_row, range.start_col),
        (range.end_row, range.end_col),
    );
    if start <= end {
        (start, end)
    } else {
        (end, start)
    }
}

/// A grid index as a reader counts it.
pub(super) fn one_based(index: u16) -> u32 {
    u32::from(index) + 1
}
