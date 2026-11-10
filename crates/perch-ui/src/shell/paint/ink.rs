//! The palette a terminal run is drawn in: the frame's colours, and the wash over a cell.

use perch_term::Color as TermColor;
use rune::Color;
use rune_kit::Kit;

/// A cell colour as the painter draws it; the frame's default is the kit's body ink.
pub(super) fn ink(color: TermColor, kit: &Kit) -> Color {
    match color {
        TermColor::Default => kit.c.text_base,
        TermColor::Rgb(r, g, b) => rgb(r, g, b),
        TermColor::Indexed(index) => indexed(index),
    }
}

/// What a selected cell is washed with: the text ink, mostly transparent, so the frame's own
/// colours still read through it in either theme.
pub(super) fn halo(kit: &Kit) -> Color {
    Color {
        a: 0.22,
        ..kit.c.text_base
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
