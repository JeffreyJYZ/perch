//! The read-only preview: a language label, numbered lines, and the Zed handoff.
//!
//! Files are read-only in perch. The preview shows what the selected file contains and offers
//! one action — open it in Zed — while a file git reports deleted shows the reason in words
//! instead of an empty page. Long files are drawn up to [`RENDER_LINES`] and say so, because
//! thousands of text elements make a frame slow for no reading benefit.

use std::path::Path;

use perch_files::{PREVIEW_MAX_LINES, PreviewText};
use rune::{El, col, row, spacer, text};
use rune_kit::{Button, ButtonVariant, Kit};

use crate::files::{FilePanel, note, section_label};

/// How many lines of a preview the view draws.
const RENDER_LINES: usize = 400;

/// A preview's lines, numbered from one.
pub fn preview_lines(text: &PreviewText) -> Vec<(usize, String)> {
    text.lines
        .iter()
        .enumerate()
        .map(|(index, line)| (index + 1, line.clone()))
        .collect()
}

/// The preview panel: its header, its toolbar and the selected file's numbered lines.
pub fn preview_view(panel: &FilePanel) -> El {
    let kit = Kit::neutral(true);
    let mut view = col()
        .key("preview")
        .role("region")
        .label("Preview")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .px(20.0)
        .py(16.0)
        .gap(6.0)
        .child(header(&kit, panel))
        .child(toolbar(&kit, panel));
    match (panel.selected_path(), panel.selected()) {
        (Some(_), Some(preview)) => view = view.child(body(&kit, preview)),
        _ => {
            let message = panel
                .message()
                .map_or_else(|| "Select a file to preview it.".to_string(), str::to_owned);
            view = view.child(text(message).font(kit.ty.body).color(kit.c.text_muted));
        }
    }
    view
}

/// The panel's label and the selected file's language, as words.
fn header(kit: &Kit, panel: &FilePanel) -> El {
    let language = panel
        .selected()
        .and_then(|preview| preview.language.as_deref())
        .map_or("plain text", language_label);
    row()
        .key("preview-header")
        .w_full()
        .justify_between()
        .gap(8.0)
        .child(section_label(kit, "Preview", "preview-label"))
        .child(text(language).font(kit.ty.small).color(kit.c.text_faint))
}

/// The selected path and the one action the panel offers: hand it to Zed.
///
/// The button carries the request's path in its tooltip and is disabled without a preview, so
/// nothing ever asks Zed to open a file that is not there. The press itself belongs to the
/// shell that owns the panel and knows how to reach `open -a Zed`.
fn toolbar(kit: &Kit, panel: &FilePanel) -> El {
    let path = panel
        .selected_path()
        .map_or_else(|| "No file selected".to_string(), display);
    let mut button = Button::new("Open in Zed").variant(ButtonVariant::GhostMuted);
    button = match panel.open_in_zed_request() {
        Some(target) => button.help(format!("Open {} in Zed", display(&target))),
        None => button.disabled(true),
    };
    row()
        .key("preview-toolbar")
        .w_full()
        .gap(8.0)
        .center()
        .child(text(path).font(kit.ty.mono).color(kit.c.text_faint))
        .child(spacer())
        .child(button.build(kit))
}

/// The numbered lines, capped so one huge file cannot cost a frame its budget.
fn body(kit: &Kit, preview: &PreviewText) -> El {
    let lines = preview_lines(preview);
    let total = lines.len();
    let shown = total.min(RENDER_LINES);
    let mut column = col().key("preview-lines").w_full().gap(0.0);
    if shown == 0 {
        return column.child(note(kit, "The file is empty."));
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
    column
}

/// One line: its number, then its text, both in the same mono face so the column aligns.
fn line_view(kit: &Kit, number: usize, line: &str) -> El {
    row()
        .key(format!("preview-line-{number}"))
        .w_full()
        .gap(12.0)
        .child(
            text(format!("{number:>5}"))
                .font(kit.ty.mono)
                .color(kit.c.text_faint),
        )
        .child(
            text(line.to_owned())
                .font(kit.ty.mono)
                .color(kit.c.text_base),
        )
}

/// The language as a word to show; an unknown name is shown as the preview spelled it.
fn language_label(language: &str) -> &str {
    if language.is_empty() {
        "plain text"
    } else {
        language
    }
}

fn display(path: &Path) -> String {
    path.display().to_string()
}
