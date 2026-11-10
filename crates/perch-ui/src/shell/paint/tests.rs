//! The painter's cell maths: where a mark falls, how wide a row is, whether the cursor draws.

use super::{cols_used, cursor_cell, marked, selection_cols};
use crate::terminal_view::paint_rows;
use perch_term::{CellRange, replay};

#[test]
fn the_cursor_is_drawn_only_where_the_frame_says_it_is() {
    let frame = replay(b"hi", 10, 2);
    assert_eq!(
        cursor_cell(&frame),
        Some((frame.cursor.row, frame.cursor.col))
    );

    // A program that hid the cursor has it drawn nowhere: a block over its output would be the
    // panel pretending the caret is there.
    let mut hidden = frame.clone();
    hidden.cursor.visible = false;
    assert_eq!(cursor_cell(&hidden), None);

    // A cursor the frame reports outside its own grid is not there to draw.
    let mut outside = frame;
    outside.cursor.col = outside.cols;
    assert_eq!(cursor_cell(&outside), None);
}

#[test]
fn a_selection_names_the_columns_it_covers_on_each_row() {
    let range = CellRange::new(1, 4, 3, 6);
    assert_eq!(selection_cols(range, 0, 20), None, "above the range");
    assert_eq!(selection_cols(range, 1, 20), Some((4, 19)));
    assert_eq!(selection_cols(range, 2, 20), Some((0, 19)));
    assert_eq!(selection_cols(range, 3, 20), Some((0, 6)));
    assert_eq!(selection_cols(range, 4, 20), None, "below it");
    // A drag that ran backwards covers the same cells.
    let backwards = CellRange::new(3, 6, 1, 4);
    assert_eq!(selection_cols(backwards, 1, 20), Some((4, 19)));
}

#[test]
fn a_marked_range_covers_whole_glyphs_and_blank_cells_alike() {
    // One blank at column 2, then 日 across 3 and 4, 本 across 5 and 6.
    let frame = replay("ab 日本".as_bytes(), 10, 1);
    let row = &frame.rows[0];
    let run = &paint_rows(&frame)[0];

    let span = marked(row, run, 0, 1).expect("a span over the first two cells");
    assert_eq!(&run.text[span.start as usize..span.end as usize], "ab");
    let span = marked(row, run, 2, 2).expect("a span over the blank cell");
    assert_eq!(&run.text[span.start as usize..span.end as usize], " ");

    // A wide glyph is one character over two cells, and its spacer names the same glyph rather
    // than half of it, so a selection that starts there copies the whole character.
    let span = marked(row, run, 3, 3).expect("a span over the wide glyph");
    assert_eq!(&run.text[span.start as usize..span.end as usize], "日");
    let span = marked(row, run, 4, 4).expect("a span over its spacer");
    assert_eq!(&run.text[span.start as usize..span.end as usize], "日");
    let span = marked(row, run, 5, 6).expect("a span over the second glyph");
    assert_eq!(&run.text[span.start as usize..span.end as usize], "本");
}

#[test]
fn a_row_knows_how_many_columns_it_used() {
    let frame = replay(b"abc", 10, 1);
    assert_eq!(cols_used(&frame.rows[0]), 3, "trailing blanks are not used");
}
