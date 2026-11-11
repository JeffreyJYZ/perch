//! perch's own kit: warm, ruled, and deliberately not the default component palette.
//!
//! The "developer tool" palette every library ships resolves to the same slate-and-green, which is
//! why a window built from it reads as generic. This one is three colours — a warm near-black, a
//! paper cream, one amber signal — and everything else is derived from them by
//! [`Kit::from_palette`], so every surface, border, hover and selection shares one temperature.
//!
//! The colours are OKLCH, which is how `docs/design/direction.md` states them: lightness first, so
//! the two themes differ in lightness and not in hue, and the signal colour keeps its chroma in
//! both.

use rune::{Color, TextStyle, oklch};
use rune_kit::{Kit, PaletteSeed};

/// The canvas: warm charcoal, not blue-grey.
fn dark_bg() -> Color {
    oklch(0.185, 0.012, 62.0)
}

/// Paper cream. Warmer than white, which is what stops the window reading as a terminal theme.
fn dark_ink() -> Color {
    oklch(0.93, 0.018, 82.0)
}

/// The one signal colour: in flight, needs you, focus.
fn dark_signal() -> Color {
    oklch(0.74, 0.15, 62.0)
}

/// The light canvas: unbleached paper rather than white.
fn light_bg() -> Color {
    oklch(0.96, 0.008, 80.0)
}

fn light_ink() -> Color {
    oklch(0.26, 0.014, 62.0)
}

fn light_signal() -> Color {
    oklch(0.55, 0.13, 62.0)
}

/// The kit for `dark`, with the type scale this app uses.
///
/// One scale, and only this: 10 for uppercase labels, 11 for secondary words, 12 for code and
/// paths, 13 for prose. Hierarchy comes from weight and ink, never from inventing a size.
pub fn perch(dark: bool) -> Kit {
    let seed = if dark {
        PaletteSeed {
            bg: dark_bg(),
            fg: dark_ink(),
            accent: dark_signal(),
        }
    } else {
        PaletteSeed {
            bg: light_bg(),
            fg: light_ink(),
            accent: light_signal(),
        }
    };
    let mut kit = Kit::from_palette(seed);
    kit.dark = dark;
    kit.ty.small = TextStyle::new(10.0, 400.0).lh(13.0);
    kit.ty.small_medium = TextStyle::new(10.0, 500.0).lh(13.0);
    kit.ty.body = TextStyle::new(13.0, 400.0).lh(19.0);
    kit.ty.body_medium = TextStyle::new(13.0, 500.0).lh(19.0);
    kit.ty.text_small = TextStyle::new(12.0, 400.0).lh(17.0);
    kit.ty.text_small_medium = TextStyle::new(12.0, 500.0).lh(17.0);
    kit.ty.mono = TextStyle::new(12.0, 400.0).mono().lh(18.0);
    kit
}

#[cfg(test)]
mod tests {
    use super::{dark_bg, perch};
    use rune_kit::Kit;

    #[test]
    fn the_kit_is_warm_and_not_the_neutral_one() {
        let dark = perch(true);
        let neutral = Kit::neutral(true);
        assert_ne!(
            dark.c.bg_deep, neutral.c.bg_deep,
            "the app's kit must not be the library's default palette"
        );
        assert_eq!(dark.c.bg_base, dark_bg());
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

    #[test]
    fn the_type_scale_has_no_other_sizes() {
        let kit = perch(true);
        assert_eq!(kit.ty.small.size, 10.0);
        assert_eq!(kit.ty.text_small.size, 12.0);
        assert_eq!(kit.ty.body.size, 13.0);
        assert_eq!(kit.ty.mono.size, 12.0);
    }
}
