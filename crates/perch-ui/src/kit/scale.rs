//! The type scale, and the one size a person picks.
//!
//! One scale and only this: 10 for uppercase labels, 12 for code and paths, 13 for prose.
//! Hierarchy comes from weight and ink, never from inventing a size — which is why a picked size
//! multiplies the scale rather than replacing one entry of it.

use rune::TextStyle;
use rune_kit::Kit;

/// The size the scale is written at: 13 for prose.
pub const BASE: f32 = 13.0;

/// The smallest size the scale may be drawn at, in points.
pub const MIN_SIZE: f32 = 10.0;

/// The largest size the scale may be drawn at, in points.
pub const MAX_SIZE: f32 = 20.0;

/// `size` inside the bounds, and the one line a pane says when it was not.
pub fn clamp_size(size: f32) -> (f32, Option<String>) {
    if !size.is_finite() {
        return (
            BASE,
            Some(format!("font size is not a number; drawn at {BASE}")),
        );
    }
    if size < MIN_SIZE {
        return (
            MIN_SIZE,
            Some(format!(
                "font size {size} is under {MIN_SIZE}; drawn at {MIN_SIZE}"
            )),
        );
    }
    if size > MAX_SIZE {
        return (
            MAX_SIZE,
            Some(format!(
                "font size {size} is over {MAX_SIZE}; drawn at {MAX_SIZE}"
            )),
        );
    }
    (size, None)
}

/// The app's own scale, byte for byte, applied to a kit that may not have one.
///
/// Every size the app sets is here, so a kit built by `perch` carries exactly the sizes the
/// direction doc states and nothing else.
pub(crate) fn type_scale(kit: &mut Kit, font_size: Option<f32>) {
    defaults(kit);
    let Some(size) = font_size else { return };
    let (size, _) = clamp_size(size);
    let ratio = size / BASE;
    if ratio == 1.0 {
        return;
    }
    kit.ty.ui = scaled(kit.ty.ui, ratio);
    kit.ty.ui_medium = scaled(kit.ty.ui_medium, ratio);
    kit.ty.ui_compact = scaled(kit.ty.ui_compact, ratio);
    kit.ty.small = scaled(kit.ty.small, ratio);
    kit.ty.small_medium = scaled(kit.ty.small_medium, ratio);
    kit.ty.body = scaled(kit.ty.body, ratio);
    kit.ty.body_medium = scaled(kit.ty.body_medium, ratio);
    kit.ty.text_small = scaled(kit.ty.text_small, ratio);
    kit.ty.text_small_medium = scaled(kit.ty.text_small_medium, ratio);
    kit.ty.text_xsmall = scaled(kit.ty.text_xsmall, ratio);
    kit.ty.text_xsmall_medium = scaled(kit.ty.text_xsmall_medium, ratio);
    kit.ty.mono = scaled(kit.ty.mono, ratio);
}

fn defaults(kit: &mut Kit) {
    kit.ty.small = TextStyle::new(10.0, 400.0).lh(13.0);
    kit.ty.small_medium = TextStyle::new(10.0, 500.0).lh(13.0);
    kit.ty.body = TextStyle::new(13.0, 400.0).lh(19.0);
    kit.ty.body_medium = TextStyle::new(13.0, 500.0).lh(19.0);
    kit.ty.text_small = TextStyle::new(12.0, 400.0).lh(17.0);
    kit.ty.text_small_medium = TextStyle::new(12.0, 500.0).lh(17.0);
    kit.ty.mono = TextStyle::new(12.0, 400.0).mono().lh(18.0);
}

/// The same style at `ratio` its size, with a line height of 0 left alone: 0 already means 1.25
/// times the size, so scaling it is the size's job.
fn scaled(style: TextStyle, ratio: f32) -> TextStyle {
    TextStyle {
        size: style.size * ratio,
        line_height: if style.line_height > 0.0 {
            style.line_height * ratio
        } else {
            0.0
        },
        ..style
    }
}
