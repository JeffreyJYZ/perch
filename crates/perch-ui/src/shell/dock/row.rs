//! One row of the dock's grid, and the reasons a grid has no rows to draw.
//!
//! A row is a line of runs with the cursor and the selection drawn over them. The cursor past the
//! row's text sits on a blank cell, which no run draws, so the row is padded out to the column it
//! is in and the block is placed there.

use perch_term::{CellRange, Frame};
use rune::{El, col, row, text};
use rune_kit::Kit;

use super::super::paint::{Over, cols_used, cursor_block, selection_cols, terminal_text};
use super::words;
use crate::files::note;
use crate::terminal_view::RowRun;

/// The dock's grid with nothing to draw, saying why.
pub(super) fn rows_note(kit: &Kit, words: &str) -> El {
    col()
        .key("terminal-rows")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .child(note(kit, words))
}

/// One row of the grid: its runs side by side, keyed by the row index so a repaint keeps the
/// lines above it, with the cursor and the selection drawn over them.
pub(super) fn terminal_row(
    index: u16,
    runs: &[RowRun],
    frame: &Frame,
    cursor: Option<(u16, u16)>,
    selection: Option<CellRange>,
    kit: &Kit,
) -> El {
    // Every row is one line of the grid, blank or not: a row's height is its font's line height,
    // never its text's, so a line with nothing on it still holds the line it is.
    let height = row_height(kit);
    let Some(cells) = frame.rows.get(usize::from(index)) else {
        return row().key(format!("terminal-row-{index}")).h(height);
    };
    let cursor = cursor.filter(|(at, _)| *at == index).map(|(_, col)| col);
    let over = Over {
        row: cells,
        cursor,
        selected: selection.and_then(|range| selection_cols(range, index, frame.cols)),
    };
    let mut line = row()
        .key(format!("terminal-row-{index}"))
        .role("row")
        .w_full()
        .h(height)
        .gap(0.0);
    // A reader hears the row's whole line, not one node per styled stretch of it: the runs are
    // cells of one line, so the label is the line and the runs say nothing of their own.
    let said = words::line(cells);
    let blank = said.is_empty();
    line = if blank {
        // A blank line has no text to read; it stays in the tree only to carry the caret.
        match cursor {
            Some(_) => line.label(words::cursor(frame)),
            None => line.a11y_hidden(),
        }
    } else {
        line.label(said)
    };
    if let Some(col) = cursor.filter(|_| !blank) {
        line = line.description(format!("cursor at column {}", words::one_based(col)));
    }
    for run in runs {
        line = line.child(terminal_text(run, over, kit));
    }
    if let Some(col) = cursor {
        let used = cols_used(cells);
        if col >= used {
            let gap = usize::from(col - used);
            if gap > 0 {
                line = line.child(text(" ".repeat(gap)).font(kit.ty.mono).a11y_hidden());
            }
            line = line.child(cursor_block(kit));
        }
    }
    line
}

/// One row of the terminal's own font, in points: how a wheel delta becomes rows of history.
pub(super) fn row_height(kit: &Kit) -> f32 {
    if kit.ty.mono.line_height > 0.0 {
        kit.ty.mono.line_height
    } else {
        kit.ty.mono.size * 1.25
    }
}
