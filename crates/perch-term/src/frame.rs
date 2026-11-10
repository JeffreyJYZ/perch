//! Painter-facing frame built from libghostty's render state.
//!
//! The UI never touches a [`Terminal`]: it receives a [`Frame`], which is plain
//! owned data and therefore `Send`. Building one costs a render-state update
//! plus one pass over the visible cells.

use libghostty_vt::{
    Error, RenderState, Terminal,
    render::{CellIteration, CellIterator, RowIterator},
    screen::CellWide,
    style::{StyleColor, Underline},
};

/// Scrollback rows the emulator keeps behind the viewport.
pub const MAX_SCROLLBACK: usize = 10_000;

/// A colour as the emulator reports it.
///
/// [`Color::Indexed`] is the raw palette entry from the style, not the palette
/// resolved to RGB, so a theme repaint can follow the palette instead of
/// freezing one theme's colours into a frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Color {
    /// Whatever the painter's default foreground/background is.
    #[default]
    Default,
    /// A 256-colour palette entry.
    Indexed(u8),
    /// A direct RGB colour.
    Rgb(u8, u8, u8),
}

impl Color {
    fn from_style(color: StyleColor) -> Self {
        match color {
            StyleColor::None => Self::Default,
            StyleColor::Palette(index) => Self::Indexed(index.0),
            StyleColor::Rgb(rgb) => Self::Rgb(rgb.r, rgb.g, rgb.b),
        }
    }
}

/// One grid cell: its text (a whole grapheme cluster) and its style.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CellView {
    /// The cell's text. Empty for a blank cell and for the tail of a wide
    /// character, which occupies two columns but holds no glyph of its own.
    pub text: String,
    /// How the cell takes part in the grid.
    ///
    /// The painter needs this to place text by column: a wide glyph covers two
    /// columns but is one character, and the empty cell after it must not be
    /// mistaken for a space.
    pub kind: CellKind,
    /// Foreground colour.
    pub fg: Color,
    /// Background colour.
    pub bg: Color,
    /// Bold.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
    /// Any underline style, flattened to a bool because the painter draws one.
    pub underline: bool,
    /// Inverse video: the painter swaps this cell's foreground and background.
    pub inverse: bool,
}

/// Whether a cell is a normal column, a wide glyph, or the spacer after one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CellKind {
    /// A one-column cell.
    #[default]
    Narrow,
    /// The leading half of a wide glyph, covering this column and the next.
    Wide,
    /// The spacer that follows a wide glyph. Draws nothing.
    Spacer,
}

/// One visible row of cells.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RowView {
    /// Every column of the row, left to right.
    pub cells: Vec<CellView>,
    /// Whether this row continues onto the next one (a soft wrap).
    pub wrapped: bool,
}

/// Where the caret is, in viewport coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CursorView {
    /// Row, 0-based from the top of the viewport.
    pub row: u16,
    /// Column, 0-based from the left.
    pub col: u16,
    /// Whether the cursor should be drawn at all.
    pub visible: bool,
}

/// A complete snapshot of the visible screen.
///
/// The plan lists both `rows: Vec<RowView>` and `rows: u16` here, which cannot compile: `rows`
/// is the cells, the height is [`Frame::height`], and it always equals `rows.len()`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Frame {
    /// One entry per visible row, top to bottom.
    pub rows: Vec<RowView>,
    /// Where the cursor sits in the viewport.
    pub cursor: CursorView,
    /// Viewport width in columns.
    pub cols: u16,
    /// Viewport height in rows.
    pub height: u16,
}

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
