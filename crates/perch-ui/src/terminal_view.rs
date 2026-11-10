//! Painting a terminal frame: cells collapsed into runs a renderer can draw.
//!
//! A frame arrives as every cell of every visible row. Handing that to a UI as
//! thousands of text nodes is what makes an embedded terminal slow, so
//! [`paint_rows`] merges neighbours that share a style into one run and drops
//! the blank tail of each row. The result is usually one run per coloured
//! stretch, which is a few dozen elements per screen instead of a grid.

use perch_term::{CellKind, CellRange, CellView, Color, Frame, RowView, selection_text};

/// A stretch of one row whose cells all share one style.
///
/// `text` is what to draw and `(row, col)` is where it starts in the grid, so
/// the painter places a run by column rather than by measuring the text it has
/// already drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowRun {
    /// Row of the frame the run belongs to, 0-based from the top.
    pub row: u16,
    /// First column the run covers, 0-based from the left.
    pub col: u16,
    /// The run's text, with the blank cells inside it as spaces.
    pub text: String,
    /// Foreground colour.
    pub fg: Color,
    /// Background colour.
    pub bg: Color,
    /// Bold.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
    /// Underlined.
    pub underline: bool,
    /// Inverse video: the painter draws `fg` on `bg` swapped.
    pub inverse: bool,
}

/// What one run paints, once inverse video is applied.
///
/// The frame reports a pair of colours and a flag; the painter draws what this says, so a run
/// drawn inverse never has to be special-cased at the drawing site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drawn {
    /// The colour the run's text is drawn in.
    pub fg: Color,
    /// The colour drawn behind it, or [`Color::Default`] for the frame's own background.
    pub bg: Color,
    /// Whether a line is drawn under the run's text.
    pub underline: bool,
}

/// Collapse a frame's cells into runs, row by row.
///
/// Trailing blank cells are dropped because the painter's own background
/// already covers them, and a blank cell with a background colour is kept
/// because that colour is something to draw.
#[must_use]
pub fn paint_rows(frame: &Frame) -> Vec<RowRun> {
    let mut runs = Vec::new();
    for (index, row) in frame.rows.iter().enumerate() {
        let Some(row_index) = u16::try_from(index).ok() else {
            break;
        };
        paint_row(row_index, row, &mut runs);
    }
    runs
}

fn paint_row(row_index: u16, row: &RowView, runs: &mut Vec<RowRun>) {
    let mut open: Option<RowRun> = None;
    for (column, cell) in row.cells.iter().take(visible_width(row)).enumerate() {
        let Some(col) = u16::try_from(column).ok() else {
            break;
        };
        let continues = open.as_ref().is_some_and(|run| shares_style(run, cell));
        if !continues && let Some(run) = open.take() {
            runs.push(run);
        }
        let run = open.get_or_insert_with(|| RowRun::start(row_index, col, cell));
        match cell.kind {
            // The tail of a wide glyph covers a column but draws nothing, and
            // the glyph before it already spans both.
            CellKind::Spacer => {}
            // A blank cell inside a run is a column the text must skip over.
            CellKind::Narrow if cell.text.is_empty() => run.text.push(' '),
            CellKind::Narrow | CellKind::Wide => run.text.push_str(&cell.text),
        }
    }
    if let Some(run) = open.take() {
        runs.push(run);
    }
}

impl RowRun {
    /// What this run draws: its colours with inverse video applied.
    ///
    /// Inverse video is the whole of the swap — a cell that asks for it draws the colour it
    /// reports as its foreground behind its text. Both colours stay unresolved, so
    /// [`Color::Default`] still means the painter's own background: an inverse cell that reports
    /// no colour at all has nothing to swap, and draws as plain text.
    #[must_use]
    pub fn drawn(&self) -> Drawn {
        let (fg, bg) = if self.inverse {
            (self.bg, self.fg)
        } else {
            (self.fg, self.bg)
        };
        Drawn {
            fg,
            bg,
            underline: self.underline,
        }
    }

    fn start(row: u16, col: u16, cell: &CellView) -> Self {
        Self {
            row,
            col,
            text: String::new(),
            fg: cell.fg,
            bg: cell.bg,
            bold: cell.bold,
            italic: cell.italic,
            underline: cell.underline,
            inverse: cell.inverse,
        }
    }
}

fn shares_style(run: &RowRun, cell: &CellView) -> bool {
    run.fg == cell.fg
        && run.bg == cell.bg
        && run.bold == cell.bold
        && run.italic == cell.italic
        && run.underline == cell.underline
        && run.inverse == cell.inverse
}

/// Columns up to and including the last one that draws something.
fn visible_width(row: &RowView) -> usize {
    row.cells
        .iter()
        .rposition(|cell| !is_blank(cell))
        .map_or(0, |last| last + 1)
}

fn is_blank(cell: &CellView) -> bool {
    cell.kind != CellKind::Wide && cell.text.is_empty() && cell.bg == Color::Default
}

/// Somewhere a copied selection can go.
///
/// The shell owns the clipboard and rune carries the write out after the handler returns, so
/// the copy is a request rather than a call. This trait is that seam: [`rune::Tx`] implements
/// it, and a test can record the text instead of touching the real clipboard.
pub trait Clipboard {
    /// Put `text` on the clipboard.
    fn copy(&mut self, text: &str);
}

impl Clipboard for rune::Tx {
    fn copy(&mut self, text: &str) {
        rune::Tx::copy(self, text);
    }
}

/// Copy the text `range` covers, and return it so the caller can say what it copied.
///
/// A blank selection is a drag over empty grid, so it writes nothing: putting an empty string
/// on the clipboard would replace whatever it already held with nothing.
pub fn copy_selection(frame: &Frame, range: CellRange, clipboard: &mut impl Clipboard) -> String {
    let text = selection_text(frame, range);
    if !text.is_empty() {
        clipboard.copy(&text);
    }
    text
}
