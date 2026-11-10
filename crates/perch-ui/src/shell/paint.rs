//! What one run of a terminal row looks like: its text, colour, background, weight and slant.
//!
//! A run is drawn twice over: its own style, and what sits on top of the cells a drag or the
//! cursor names. The cell maths lives in [`over`] and the colour table in [`ink`], so this file
//! is only the element a run becomes.

mod ink;
mod over;

#[cfg(test)]
mod tests;

use perch_term::Color as TermColor;
use rune::{El, Span, TextStyle, text};
use rune_kit::Kit;

use crate::terminal_view::{Drawn, RowRun};
use ink::{halo, ink};
use over::marked;

pub(super) use over::{cols_used, cursor_cell, selection_cols};

/// What is drawn over a row's cells: the cursor cell, and the columns a drag selected.
///
/// The row's own cells come along because a span is a byte range in the run's text, and only the
/// grid knows how many bytes each cell took: a wide glyph is one cell of several bytes and the
/// spacer after it is one cell of none.
#[derive(Clone, Copy)]
pub(super) struct Over<'a> {
    /// The row the run was painted from.
    pub row: &'a perch_term::RowView,
    /// The column the cursor sits in, when it is on this row and drawn.
    pub cursor: Option<u16>,
    /// The columns the selection covers on this row, inclusive.
    pub selected: Option<(u16, u16)>,
}

/// One run of one row, drawn as the cell it came from.
///
/// A run is contiguous by construction, so a row's runs can be laid out as siblings and still
/// read as the line the shell wrote. Its background and underline ride on a span over the run's
/// own text, so a styled cell is still one element.
pub(super) fn terminal_text(run: &RowRun, over: Over<'_>, kit: &Kit) -> El {
    let drawn = run.drawn();
    let el = text(run.text.clone())
        .font(TextStyle {
            weight: if run.bold { 600.0 } else { kit.ty.mono.weight },
            italic: run.italic,
            ..kit.ty.mono
        })
        .color(ink(drawn.fg, kit))
        // A run is a stretch of one line, not a word of its own: the row it belongs to says the
        // line, so a reader hears a sentence instead of one node per colour.
        .a11y_hidden();
    let mut spans = span_for(run, &drawn, kit).into_iter().collect::<Vec<_>>();
    if let Some((from, to)) = over.selected
        && let Some(span) = marked(over.row, run, from, to)
    {
        spans.push(span.bg(halo(kit), 0.0));
    }
    if let Some(col) = over.cursor
        && let Some(span) = marked(over.row, run, col, col)
    {
        // A block cursor is the text turned inside out: the cell's ink becomes the plate.
        spans.push(span.color(kit.c.bg_base).bg(kit.c.text_base, 0.0));
    }
    if spans.is_empty() {
        return el;
    }
    el.spans(spans).span_radius(0.0)
}

/// The block cursor, for a cell the row has no glyph in: a prompt's end is the usual place.
///
/// A blank cell draws nothing, so there is no run to turn inside out; this is the plate on its
/// own, one mono space wide.
pub(super) fn cursor_block(kit: &Kit) -> El {
    text(" ")
        .font(kit.ty.mono)
        .spans([Span::new(0, 1)
            .color(kit.c.bg_base)
            .bg(kit.c.text_base, 0.0)])
        .span_radius(0.0)
        // The block is a caret, not a word: a reader hearing "space" has learned nothing.
        .a11y_hidden()
}

/// The span carrying a run's background and underline, when it has either.
///
/// A run whose background is the frame's default draws no background: the dock's own panel is
/// what shows through, and painting the default would pin one theme's colour into the frame.
fn span_for(run: &RowRun, drawn: &Drawn, kit: &Kit) -> Option<Span> {
    let mut span = Span::new(0, run.text.len());
    let mut any = false;
    if drawn.bg != TermColor::Default {
        span = span.bg(ink(drawn.bg, kit), 0.0);
        any = true;
    }
    if drawn.underline {
        span = span.underline(ink(drawn.fg, kit));
        any = true;
    }
    any.then_some(span)
}
