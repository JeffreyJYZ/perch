//! A tool call as one card: what ran, how it ended, and either the start of what it returned or
//! all of it when the card is expanded.

use perch_tools::Preview;
use rune::{Color, El, col, row, text};
use rune_kit::Kit;

use super::ToolStatus;
use super::view::speaker;
use super::words::{collapsed_output, expanded_output};

/// A tool call as one card, named for the tool and what became of it.
///
/// The card takes the focus so the window's map can open it: the root's key handler runs before
/// a focused control's own activation rules, which is what lets Enter and Space both reach it.
/// Its label carries the call and its description the status, so a reader moving by controls
/// hears both without reading the output.
pub(super) fn tool_card(
    index: usize,
    name: &str,
    status: ToolStatus,
    output: &str,
    preview: Option<&Preview>,
    expanded: bool,
    kit: &Kit,
) -> El {
    let mut card = col()
        .key(format!("tool-{index}"))
        .role("button")
        .label(format!("Tool call {name}"))
        .description(status.words())
        .focusable(true)
        .w_full()
        .gap(6.0)
        .px(12.0)
        .py(10.0)
        .radius(kit.radius.md)
        .fill(kit.c.bg_base)
        .child(
            row()
                .w_full()
                .justify_between()
                .gap(12.0)
                .child(text(name).font(kit.ty.small_medium).color(kit.c.text_base))
                .child(
                    text(status.words())
                        .font(kit.ty.small)
                        .color(status_ink(status, kit)),
                ),
        );
    if let Some(preview) = preview {
        card = card.child(preview_view(preview, kit));
    }
    if !output.is_empty() {
        let shown = if expanded {
            expanded_output(output)
        } else {
            collapsed_output(output)
        };
        card = card.child(
            text(shown)
                .font(kit.ty.mono)
                .color(kit.c.text_muted)
                .selectable(true),
        );
    }
    card
}

/// The diff or command an approval shows, whole: the user judges all of it, not five lines.
fn preview_view(preview: &Preview, kit: &Kit) -> El {
    let (label, body) = match preview {
        Preview::Diff(diff) => ("diff", diff),
        Preview::Command(command) => ("command", command),
    };
    col()
        .key("preview")
        .w_full()
        .gap(2.0)
        .child(speaker(label, kit))
        .child(
            text(body.clone())
                .font(kit.ty.mono)
                .color(kit.c.text_muted)
                .selectable(true),
        )
}

/// The status word's colour. It reinforces the word; it never carries the meaning alone.
fn status_ink(status: ToolStatus, kit: &Kit) -> Color {
    match status {
        ToolStatus::Running => kit.c.text_muted,
        ToolStatus::Ok => kit.c.success.fg,
        ToolStatus::Failed => kit.c.danger.fg,
        ToolStatus::Cancelled => kit.c.text_faint,
        ToolStatus::ApprovalNeeded => kit.c.warning.fg,
    }
}
