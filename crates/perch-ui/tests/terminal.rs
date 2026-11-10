//! The painter's view of a frame: cells collapsed into runs.

use perch_term::replay;
use perch_ui::paint_rows;

#[test]
fn identical_styles_collapse_into_one_run() {
    let frame = replay(b"aaa\x1b[1mbbb", 10, 1);
    let runs = paint_rows(&frame);

    assert_eq!(runs[0].text, "aaa");
    assert!(
        runs.iter().any(|run| run.text == "bbb" && run.bold),
        "the bold run is its own run: {runs:?}"
    );
    assert_eq!(runs.len(), 2, "trailing blank cells make no run: {runs:?}");
    assert_eq!((runs[1].row, runs[1].col), (0, 3), "a run knows its column");
}

#[test]
fn blank_cells_inside_a_run_stay_as_spaces_and_wide_glyphs_keep_their_columns() {
    let frame = replay("a  b 日本".as_bytes(), 12, 1);
    let runs = paint_rows(&frame);

    assert_eq!(runs.len(), 1, "one style is one run: {runs:?}");
    // Two blanks are two columns, and the tail of each wide glyph is one
    // column of the glyph itself, not a space.
    assert_eq!(runs[0].text, "a  b 日本");
    assert_eq!(runs[0].col, 0);
}
