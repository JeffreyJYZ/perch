//! The owned data a painter draws: colours, cells, rows, the cursor and the frame.

use libghostty_vt::style::StyleColor;

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
    pub(super) fn from_style(color: StyleColor) -> Self {
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
