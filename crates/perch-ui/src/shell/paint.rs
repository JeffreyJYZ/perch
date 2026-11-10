//! What one run of a terminal row looks like: its text, colour, background, weight and slant.

use perch_term::Color as TermColor;
use rune::{Color, El, Span, TextStyle, text};
use rune_kit::Kit;

use crate::terminal_view::{Drawn, RowRun};

/// One run of one row, drawn as the cell it came from.
///
/// A run is contiguous by construction, so a row's runs can be laid out as siblings and still
/// read as the line the shell wrote. Its background and underline ride on a span over the run's
/// own text, so a styled cell is still one element.
pub(super) fn terminal_text(run: &RowRun, kit: &Kit) -> El {
    let drawn = run.drawn();
    let mut el = text(run.text.clone())
        .font(TextStyle {
            weight: if run.bold { 600.0 } else { kit.ty.mono.weight },
            italic: run.italic,
            ..kit.ty.mono
        })
        .color(ink(drawn.fg, kit));
    if let Some(span) = span_for(run, &drawn, kit) {
        el = el.spans([span]).span_radius(0.0);
    }
    el
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

/// A cell colour as the painter draws it; the frame's default is the kit's body ink.
fn ink(color: TermColor, kit: &Kit) -> Color {
    match color {
        TermColor::Default => kit.c.text_base,
        TermColor::Rgb(r, g, b) => rgb(r, g, b),
        TermColor::Indexed(index) => indexed(index),
    }
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color {
        r: f32::from(r) / 255.0,
        g: f32::from(g) / 255.0,
        b: f32::from(b) / 255.0,
        a: 1.0,
    }
}

/// A 256-colour palette entry, as xterm defines it: sixteen named colours, a 6×6×6 cube, then
/// twenty-four greys. The frame carries the palette index, so the table lives at the painter.
fn indexed(index: u8) -> Color {
    const BASE: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (205, 49, 49),
        (13, 188, 121),
        (229, 229, 16),
        (36, 114, 200),
        (188, 63, 188),
        (17, 168, 205),
        (229, 229, 229),
        (102, 102, 102),
        (241, 76, 76),
        (35, 209, 139),
        (245, 245, 67),
        (59, 142, 234),
        (214, 112, 214),
        (41, 184, 219),
        (255, 255, 255),
    ];
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    match index {
        0..=15 => {
            let (r, g, b) = BASE[usize::from(index)];
            rgb(r, g, b)
        }
        16..=231 => {
            let cube = index - 16;
            rgb(
                LEVELS[usize::from(cube / 36)],
                LEVELS[usize::from((cube % 36) / 6)],
                LEVELS[usize::from(cube % 6)],
            )
        }
        _ => {
            let grey = 8 + (index - 232) * 10;
            rgb(grey, grey, grey)
        }
    }
}
