//! What a selection copies: trimmed rows, whole wide glyphs, and soft wraps joined back up.

use perch_term::{CellRange, replay, selection_text};

#[test]
fn each_row_is_trimmed_and_rows_are_joined_with_a_newline() {
    let frame = replay(b"ab\r\ncd", 6, 2);
    assert_eq!(selection_text(&frame, CellRange::new(0, 0, 1, 5)), "ab\ncd");
    // The same range dragged backwards is the same selection.
    assert_eq!(selection_text(&frame, CellRange::new(1, 5, 0, 0)), "ab\ncd");
}

#[test]
fn a_range_inside_one_row_copies_only_that_column_span() {
    let frame = replay(b"one two three", 14, 1);
    assert_eq!(selection_text(&frame, CellRange::new(0, 4, 0, 6)), "two");
}

#[test]
fn a_wide_glyph_is_copied_whole_even_from_its_second_column() {
    let frame = replay("日本語".as_bytes(), 10, 1);
    assert_eq!(selection_text(&frame, CellRange::new(0, 0, 0, 9)), "日本語");
    // Column 1 is the spacer half of 日: the glyph is still one character, not half of one.
    assert_eq!(selection_text(&frame, CellRange::new(0, 1, 0, 1)), "日");
}

#[test]
fn a_soft_wrapped_row_is_joined_to_the_next_without_a_newline() {
    let frame = replay(b"abcdefghij", 5, 2);
    assert!(frame.rows[0].wrapped, "the first row soft-wraps");
    assert_eq!(
        selection_text(&frame, CellRange::new(0, 0, 1, 4)),
        "abcdefghij"
    );
}

#[test]
fn a_range_that_outlives_a_resize_stops_at_the_grid() {
    // A selection made before a resize can name rows and columns this frame no longer has.
    let frame = replay(b"only", 4, 2);
    assert_eq!(selection_text(&frame, CellRange::new(0, 0, 9, 9)), "only");
}
