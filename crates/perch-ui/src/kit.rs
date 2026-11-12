//! perch's own kit: warm, ruled, and deliberately not the default component palette.
//!
//! The "developer tool" palette every library ships resolves to the same slate-and-green, which is
//! why a window built from it reads as generic. This one is three colours — a warm near-black, a
//! paper cream, one amber signal — and everything else is derived from them by
//! [`Kit::from_palette`], so every surface, border, hover and selection shares one temperature.
//!
//! The colours are OKLCH, which is how `docs/design/direction.md` states them: lightness first, so
//! the two themes differ in lightness and not in hue, and the signal colour keeps its chroma in
//! both. A theme from `$PERCH_HOME/.config/perch/themes` names the same three anchors, and the two
//! states it may; [`from_anchors`] is the one place a kit is built from either.

mod anchors;
mod scale;

use rune::Color;
use rune_kit::{Kit, StateColors};

pub use anchors::{Anchors, builtin};
pub use scale::{BASE, MAX_SIZE, MIN_SIZE, clamp_size};

/// The kit for `dark`, with the type scale this app uses.
pub fn perch(dark: bool) -> Kit {
    from_anchors(&builtin(dark), None)
}

/// The kit a theme draws: its anchors, and the type size a person picked.
///
/// `font_size` absent draws today's scale exactly, which is what every caller that names no size
/// gets; a size is clamped to [`MIN_SIZE`]..=[`MAX_SIZE`] before it scales anything, so no caller
/// can hand the frame text it cannot lay out.
pub fn from_anchors(anchors: &Anchors, font_size: Option<f32>) -> Kit {
    let mut kit = Kit::from_palette(anchors.seed());
    kit.dark = anchors.dark();
    if let Some(ok) = anchors.ok {
        kit.c.success = marked(kit.c.success, ok);
    }
    if let Some(fail) = anchors.fail {
        kit.c.danger = marked(kit.c.danger, fail);
    }
    scale::type_scale(&mut kit, font_size);
    kit
}

/// A state painted in `tone` instead of the library's own green or red.
///
/// The wash under it stays the derived one: a theme names a colour, and does not thereby own every
/// tint of it.
fn marked(state: StateColors, tone: Color) -> StateColors {
    StateColors {
        fg: tone,
        icon: tone,
        border: tone.alpha(0.35),
        ..state
    }
}

#[cfg(test)]
mod tests {
    use super::{builtin, perch};
    use rune_kit::Kit;

    #[test]
    fn the_kit_is_warm_and_not_the_neutral_one() {
        let dark = perch(true);
        let neutral = Kit::neutral(true);
        assert_ne!(
            dark.c.bg_deep, neutral.c.bg_deep,
            "the app's kit must not be the library's default palette"
        );
        assert_eq!(dark.c.bg_base, builtin(true).bg);
        assert!(dark.dark, "the dark kit says it is dark");
        assert!(!perch(false).dark, "and the light one says it is light");
    }

    #[test]
    fn the_two_themes_differ_in_lightness_and_not_in_hue() {
        let dark = perch(true);
        let light = perch(false);
        assert_ne!(dark.c.bg_base, light.c.bg_base);
        assert_ne!(dark.c.text_base, light.c.text_base);
        // The signal keeps its hue in both modes: an accent that changes hue between themes is a
        // second brand colour nobody chose.
        assert_ne!(dark.c.text_accent, light.c.text_accent);
    }
}
