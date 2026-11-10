//! Painter-facing frame built from libghostty's render state.
//!
//! The UI never touches a [`Terminal`]: it receives a [`Frame`], which is plain
//! owned data and therefore `Send`. Building one costs a render-state update
//! plus one pass over the visible cells.

use libghostty_vt::{
    Error, RenderState, Terminal,
    render::{CellIteration, CellIterator, RowIterator},
    screen::CellWide,
    style::Underline,
};

mod view;

pub use view::{CellKind, CellView, Color, CursorView, Frame, RowView};

/// Scrollback rows the emulator keeps behind the viewport.
pub const MAX_SCROLLBACK: usize = 10_000;

/// Reusable render state: one per terminal thread, updated per frame, never reallocated.
#[derive(Debug)]
pub struct FrameBuilder<'alloc> {
    state: RenderState<'alloc>,
    rows: RowIterator<'alloc>,
    cells: CellIterator<'alloc>,
}

impl<'alloc> FrameBuilder<'alloc> {
    /// Create the render state and the two iterators it needs.
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            state: RenderState::new()?,
            rows: RowIterator::new()?,
            cells: CellIterator::new()?,
        })
    }

    /// Snapshot the terminal's visible screen into a [`Frame`].
    pub fn build(&mut self, terminal: &Terminal<'alloc, '_>) -> Result<Frame, Error> {
        let snapshot = self.state.update(terminal)?;
        let cols = snapshot.cols()?;
        let height = snapshot.rows()?;
        let visible = snapshot.cursor_visible()?;
        let cursor = match snapshot.cursor_viewport()? {
            Some(viewport) => CursorView {
                row: viewport.y,
                col: viewport.x,
                visible,
            },
            // A cursor parked outside the viewport (scrolled off) is not drawn.
            None => CursorView::default(),
        };

        let mut rows = Vec::with_capacity(usize::from(height));
        let mut text = String::new();
        let mut row_iteration = self.rows.update(&snapshot)?;
        while let Some(row) = row_iteration.next() {
            let wrapped = row.raw_row()?.is_wrapped()?;
            let mut cells = Vec::with_capacity(usize::from(cols));
            let mut cell_iteration = self.cells.update(row)?;
            while cell_iteration.next().is_some() {
                cells.push(cell_view(&cell_iteration, &mut text)?);
            }
            rows.push(RowView { cells, wrapped });
        }

        Ok(Frame {
            rows,
            cursor,
            cols,
            height,
        })
    }
}

fn cell_view(cell: &CellIteration<'_, '_>, text: &mut String) -> Result<CellView, Error> {
    // A spacer (the tail of a wide character) and an empty cell both answer `InvalidValue`,
    // which is not an error worth reporting: the cell simply has no glyph.
    text.clear();
    if cell.graphemes_utf8(text).is_err() {
        text.clear();
    }
    // An unstyled cell reports the default style; if the render state ever
    // declines to answer, plain text still beats no frame at all.
    let style = cell.style().unwrap_or_default();
    Ok(CellView {
        text: text.clone(),
        kind: kind(cell),
        fg: Color::from_style(style.fg_color),
        bg: Color::from_style(style.bg_color),
        bold: style.bold,
        italic: style.italic,
        underline: style.underline != Underline::None,
        inverse: style.inverse,
    })
}

/// A cell's width class, read from the raw cell because the render iteration
/// reports an empty string for the spacer half of a wide glyph.
fn kind(cell: &CellIteration<'_, '_>) -> CellKind {
    match cell.raw_cell().and_then(|raw| raw.wide()) {
        Ok(CellWide::Wide) => CellKind::Wide,
        Ok(CellWide::SpacerTail | CellWide::SpacerHead) => CellKind::Spacer,
        Ok(CellWide::Narrow) | Err(_) => CellKind::Narrow,
    }
}
