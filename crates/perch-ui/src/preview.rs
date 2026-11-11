//! The read-only preview: the centre column's stand-in for the transcript.
//!
//! Files are read-only in perch. Selecting one in the tree shows its contents in the centre
//! column at full width, and the transcript is never lost: it keeps streaming while hidden, and
//! Escape — or the header's button — brings it back with the file still selected in the tree.
//! Long files are drawn up to [`RENDER_LINES`] and say so, because thousands of text elements
//! make a frame slow for no reading benefit.

mod body;
mod keys;

use std::path::Path;

use rune::{El, col, row, text};
use rune_kit::{Button, ButtonVariant, Kit};

use crate::files::{FilePanel, note, section_label};
use crate::shell::{Live, Shell};

pub use body::preview_lines;
pub use keys::preview_key;

/// How many lines of a preview the view draws.
pub const RENDER_LINES: usize = 400;

/// The preview: a header strip naming the file, its numbered lines, and the Zed handoff.
///
/// The header is a strip of its own above the body, so the path and the two actions stay put
/// while the lines scroll under them.
pub fn preview_view<L: Live>(panel: &FilePanel) -> El {
    let kit = Kit::neutral(true);
    let mut view = col()
        .key("preview")
        .role("region")
        .label("Preview")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .fill(kit.c.bg_deep)
        .child(header::<L>(&kit, panel));
    // Words about the handoff belong beside the file they are about, not in place of it: a
    // refusal keeps the preview on screen so the reader can try again.
    if panel.selected().is_some()
        && let Some(words) = panel.message()
    {
        view = view.child(note(&kit, words).px(20.0).py(4.0));
    }
    match (panel.selected_path(), panel.selected()) {
        (Some(_), Some(preview)) => view = view.child(body::lines(&kit, preview)),
        _ => {
            let message = panel
                .message()
                .map_or_else(|| "Select a file to preview it.".to_string(), str::to_owned);
            view = view.child(note(&kit, &message).px(20.0).py(4.0));
        }
    }
    view
}

/// The header strip: what is open, in what language, and the way back to the transcript.
fn header<L: Live>(kit: &Kit, panel: &FilePanel) -> El {
    let path = panel
        .selected_path()
        .map_or_else(|| "No file selected".to_string(), display);
    let language = panel
        .selected()
        .and_then(|preview| preview.language.as_deref())
        .map_or("plain text", language_label);
    let mut strip = row()
        .key("preview-header")
        .w_full()
        .gap(12.0)
        .px(20.0)
        .pt(12.0)
        .pb(8.0)
        .fill(kit.c.bg_base)
        .child(section_label(kit, "Preview", "preview-label"))
        .child(
            text(path)
                .key("preview-path")
                .grow(1.0)
                .min_w(0.0)
                .font(kit.ty.mono)
                .color(kit.c.text_base)
                .truncate_middle(),
        );
    // The cap is stated where the file is named, not only under the last drawn line: a reader
    // who never scrolls to the bottom still learns the file is longer than what is shown.
    if let Some(words) = cap(panel) {
        strip = strip.child(
            text(words)
                .key("preview-cap")
                .font(kit.ty.small)
                .color(kit.c.text_faint),
        );
    }
    strip
        .child(
            text(language)
                .key("preview-language")
                .font(kit.ty.small)
                .color(kit.c.text_faint),
        )
        .child(zed::<L>(kit, panel))
        .child(close::<L>(kit))
}

/// Where the preview stops, when the file is longer than [`RENDER_LINES`].
fn cap(panel: &FilePanel) -> Option<String> {
    let total = panel.selected()?.lines.len();
    (total > RENDER_LINES).then(|| format!("first {RENDER_LINES} of {total} lines"))
}

/// The one action the preview offers: hand the selected file to the app's editor.
///
/// The button carries the path in its tooltip and is disabled without a preview, so nothing ever
/// asks Zed to open a file that is not there. The press runs the app's [`Editor`], the only code
/// in the window that touches a command line.
///
/// [`Editor`]: crate::files::Editor
fn zed<L: Live>(kit: &Kit, panel: &FilePanel) -> El {
    let mut button = Button::new("Open in Zed").variant(ButtonVariant::GhostMuted);
    button = match panel.open_in_zed_request() {
        Some(target) => button
            .help(format!("Open {} in Zed", display(&target)))
            .on_press(|shell: &mut Shell<L>| {
                shell.open_in_zed();
            }),
        None => button.disabled(true),
    };
    button.build(kit).key("open-in-zed")
}

/// The way back: the transcript, with the tree's selection untouched.
fn close<L: Live>(kit: &Kit) -> El {
    Button::new("Back to transcript")
        .variant(ButtonVariant::GhostMuted)
        .shortcut("esc")
        .help("Leave the preview for the transcript; the file stays selected (Esc)")
        .on_press(|shell: &mut Shell<L>| {
            shell.close_preview();
        })
        .build(kit)
        .key("preview-close")
}

/// The language as a word to show; an unknown name is shown as the preview spelled it.
pub(crate) fn language_label(language: &str) -> &str {
    if language.is_empty() {
        "plain text"
    } else {
        language
    }
}

fn display(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::language_label;

    #[test]
    fn a_language_is_shown_as_its_name_and_an_empty_one_as_plain_text() {
        assert_eq!(language_label("rust"), "rust");
        assert_eq!(language_label(""), "plain text");
    }
}
