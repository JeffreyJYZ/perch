//! The painter's view of a frame: cells collapsed into runs.

use perch_term::{CellRange, replay};
use perch_ui::paint_rows;
use perch_ui::terminal_view::{Clipboard, RowRun, copy_selection};

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

#[test]
fn underline_and_inverse_are_reported_and_distinct_from_plain_cells() {
    let frame = replay(b"a\x1b[4mb\x1b[0m\x1b[7mc", 6, 1);
    let runs = paint_rows(&frame);

    let plain = run(&runs, "a");
    assert!(
        !plain.underline && !plain.inverse,
        "a plain cell draws neither: {plain:?}"
    );

    let underlined = run(&runs, "b");
    assert!(
        underlined.underline,
        "SGR 4 reaches the run: {underlined:?}"
    );
    assert!(!underlined.inverse, "{underlined:?}");

    let inverse = run(&runs, "c");
    assert!(inverse.inverse, "SGR 7 reaches the run: {inverse:?}");
    assert!(!inverse.underline, "{inverse:?}");
}

#[test]
fn a_selection_copies_once_and_a_blank_one_copies_nothing() {
    let frame = replay(b"hello world", 20, 2);
    let mut board = Recorder::default();

    let text = copy_selection(&frame, CellRange::new(0, 0, 0, 10), &mut board);
    assert_eq!(text, "hello world");
    assert_eq!(board.0, ["hello world".to_owned()]);

    // A drag over the empty second row selects nothing, and writing an empty string would
    // replace whatever the clipboard already held.
    let text = copy_selection(&frame, CellRange::new(1, 0, 1, 19), &mut board);
    assert_eq!(text, "");
    assert_eq!(
        board.0.len(),
        1,
        "a blank selection writes nothing: {:?}",
        board.0
    );
}

#[test]
fn the_shell_can_hand_a_selection_straight_to_rune() {
    let frame = replay(b"copy me", 10, 1);
    // `Tx` records the request and rune's clipboard worker carries it out after the handler.
    let text = copy_selection(&frame, CellRange::new(0, 0, 0, 6), &mut rune::Tx::default());
    assert_eq!(text, "copy me");
}

/// The one run whose text is exactly `text`.
fn run<'a>(runs: &'a [RowRun], text: &str) -> &'a RowRun {
    runs.iter()
        .find(|run| run.text == text)
        .unwrap_or_else(|| panic!("no {text:?} run in {runs:?}"))
}

/// A clipboard that keeps what it was given, so a test can read it back.
#[derive(Default)]
struct Recorder(Vec<String>);

impl Clipboard for Recorder {
    fn copy(&mut self, text: &str) {
        self.0.push(text.to_owned());
    }
}
