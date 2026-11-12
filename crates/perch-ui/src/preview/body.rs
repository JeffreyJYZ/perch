//! The preview's numbered lines, in one clipped viewport that scrolls both ways.
//!
//! No line wraps: code is drawn at its natural width and the viewport moves, so a column of code
//! keeps its shape. The viewport also clips, which is what keeps a long file inside the preview's
//! box instead of painting over the dock below it.

use perch_files::{PREVIEW_MAX_LINES, PreviewText};
use rune::{Align, El, col, row, text};
use rune_kit::Kit;

use super::RENDER_LINES;
use crate::files::note;

/// The width of the number gutter, in points: five digits of the mono face, so every number in a
/// file ends at the same right edge whatever its length.
pub(crate) const GUTTER: f32 = 48.0;

/// A preview's lines, numbered from one.
pub fn preview_lines(text: &PreviewText) -> Vec<(usize, String)> {
    text.lines
        .iter()
        .enumerate()
        .map(|(index, line)| (index + 1, line.clone()))
        .collect()
}

/// The numbered lines in their viewport, capped so one huge file cannot cost a frame its budget.
pub(super) fn lines(kit: &Kit, preview: &PreviewText) -> El {
    let lines = preview_lines(preview);
    let total = lines.len();
    let shown = total.min(RENDER_LINES);
    let mut column = col().key("preview-lines").gap(0.0).px(20.0).py(10.0);
    if shown == 0 {
        return viewport(column.child(note(kit, "The file is empty.")));
    }
    for (number, line) in lines.iter().take(shown) {
        column = column.child(line_view(kit, *number, line));
    }
    if shown < total {
        column = column.child(note(
            kit,
            &format!("Showing the first {shown} of {total} lines."),
        ));
    }
    if total >= PREVIEW_MAX_LINES {
        column = column.child(note(
            kit,
            &format!("The preview stops at {PREVIEW_MAX_LINES} lines."),
        ));
    }
    viewport(column)
}

/// The one viewport: bounded by the column above it, clipped on both axes.
fn viewport(content: El) -> El {
    col()
        .key("preview-body")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll_xy()
        .child(content)
}

/// One line: its number in the fixed gutter, then its text, both in the same mono face.
fn line_view(kit: &Kit, number: usize, line: &str) -> El {
    row()
        .key(format!("preview-line-{number}"))
        .gap(12.0)
        .child(
            text(number.to_string())
                .key(format!("preview-number-{number}"))
                .w(GUTTER)
                .align(Align::End)
                .font(kit.ty.mono)
                .color(kit.c.text_faint),
        )
        .child(
            text(line.to_owned())
                .key(format!("preview-text-{number}"))
                .font(kit.ty.mono)
                .color(kit.c.text_base),
        )
}

#[cfg(test)]
mod tests {
    use perch_files::PreviewText;

    use super::preview_lines;

    #[test]
    fn preview_lines_are_one_based_and_numbered() {
        let text = PreviewText {
            lines: vec!["a".into(), "b".into()],
            language: Some("rust".into()),
        };
        assert_eq!(
            preview_lines(&text),
            vec![(1, "a".to_string()), (2, "b".to_string())]
        );
    }
}
