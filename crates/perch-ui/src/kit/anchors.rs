//! The three anchors a theme names, plus the two states it may override.
//!
//! Everything else in the kit — every layer, hairline, hover and muted text — is derived from
//! these by `Kit::from_palette`, so a theme that could set all twenty tones would drift from the
//! type scale and the hairlines one token at a time.

use rune::{Color, oklch};
use rune_kit::PaletteSeed;

/// A theme's colours: the canvas, the ink on it, the one signal, and optionally the two states.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchors {
    pub bg: Color,
    pub ink: Color,
    pub signal: Color,
    /// What "succeeded" is drawn in, when the theme names it.
    pub ok: Option<Color>,
    /// What "failed" is drawn in, when the theme names it.
    pub fail: Option<Color>,
}

impl Anchors {
    /// The seed [`Kit::from_palette`](rune_kit::Kit::from_palette) derives every other tone from.
    pub(crate) fn seed(self) -> PaletteSeed {
        PaletteSeed {
            bg: self.bg,
            fg: self.ink,
            accent: self.signal,
        }
    }

    /// Whether the ink is lighter than the canvas, which is the whole of "dark" here.
    pub fn dark(self) -> bool {
        self.ink.luminance() > self.bg.luminance()
    }
}

/// The anchors the app ships: a neutral charcoal under paper cream, one amber signal.
///
/// The canvas is neutral rather than warm on purpose: a brown cast on a dark canvas reads as a
/// mistake, not a mood, which is why the dark canvas carries 0.002 chroma and not more.
pub fn builtin(dark: bool) -> Anchors {
    if dark {
        Anchors {
            bg: oklch(0.165, 0.002, 265.0),
            ink: oklch(0.94, 0.004, 265.0),
            signal: oklch(0.74, 0.15, 62.0),
            ok: None,
            fail: None,
        }
    } else {
        Anchors {
            bg: oklch(0.965, 0.002, 265.0),
            ink: oklch(0.22, 0.004, 265.0),
            signal: oklch(0.55, 0.13, 62.0),
            ok: None,
            fail: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Anchors, builtin};

    #[test]
    fn the_two_built_ins_differ_in_lightness_and_not_in_hue() {
        let (dark, light) = (builtin(true), builtin(false));
        assert!(dark.dark(), "cream ink on a charcoal canvas is dark");
        assert!(!light.dark());
        assert_ne!(
            dark.signal, light.signal,
            "the signal keeps its hue, not its lightness"
        );
        assert_eq!(builtin(false), light, "the anchors are values, not state");
    }

    #[test]
    fn a_theme_with_no_states_leaves_them_derived() {
        let anchors = Anchors {
            ok: None,
            fail: None,
            ..builtin(true)
        };
        assert!(anchors.ok.is_none());
        assert_eq!(anchors.seed().bg, anchors.bg);
    }
}
