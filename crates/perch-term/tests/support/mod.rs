//! Helpers shared by the terminal integration tests.

use perch_term::Frame;

/// One row's text, which is every glyph on it: blanks carry an empty string.
pub fn line(frame: &Frame, row: usize) -> String {
    frame.rows[row]
        .cells
        .iter()
        .map(|cell| cell.text.as_str())
        .collect()
}
