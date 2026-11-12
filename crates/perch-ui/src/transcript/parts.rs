//! The pieces a card draws beside its output: the status word's ink, and the diff or command an
//! approval shows whole.

use perch_tools::Preview;
use rune::{Color, El, col, text};
use rune_kit::Kit;

use super::ToolStatus;
use super::view::speaker;

/// The diff or command an approval shows, whole: the user judges all of it, not five lines.
pub(super) fn preview_view(preview: &Preview, kit: &Kit) -> El {
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
pub(super) fn status_ink(status: ToolStatus, kit: &Kit) -> Color {
    match status {
        ToolStatus::Running => kit.c.text_muted,
        ToolStatus::Ok => kit.c.success.fg,
        ToolStatus::Failed => kit.c.danger.fg,
        ToolStatus::Cancelled => kit.c.text_faint,
        ToolStatus::ApprovalNeeded => kit.c.warning.fg,
    }
}
