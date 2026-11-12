//! A tool call as one card: what ran, how it ended, and either the start of what it returned or
//! the whole of it when the card is opened.
//!
//! The card's header is its handle: a press on it opens the output where the card sits, and the
//! window's chord does the same for the card that holds the focus. What opens is the text the
//! transcript already holds, so a reopened session opens the same card with nothing re-read.

use perch_tools::Preview;
use rune::{El, col, row, text};
use rune_kit::Kit;

use super::ToolStatus;
use super::Transcript;
use super::parts::{preview_view, status_ink};
use super::recording::{Recorded, recorded};
use super::words::{collapsed_output, opened_output};

/// What a press on a card's header does.
///
/// Two states answer it: the column on its own, which is what a test lays out, and the window,
/// which holds the app's transcript as well as its copy.
pub trait CardPress: 'static {
    /// Opens or closes the tool card at `index`, whose provider id is `id`.
    fn card_press(&mut self, index: usize, id: &str);
}

impl CardPress for Transcript {
    fn card_press(&mut self, _index: usize, id: &str) {
        self.toggle(id);
    }
}

/// What one card draws, bundled so a card cannot be drawn with another call's status or target.
pub(super) struct Card<'a> {
    /// The tool name, which is the card's one-line summary.
    pub(super) name: &'a str,
    /// Where the call got to.
    pub(super) status: ToolStatus,
    /// The result text as the tool returned it, uncollapsed.
    pub(super) output: &'a str,
    /// The diff or command the approval view needs, when there is one.
    pub(super) preview: Option<&'a Preview>,
    /// The file or command the call acts on, when the provider said.
    pub(super) target: Option<&'a str>,
    /// Whether the card is showing its whole output.
    pub(super) expanded: bool,
}

/// A tool call as one card, named for the tool and what became of it.
///
/// The card takes the focus so the window's map can open it: the root's key handler runs before
/// a focused control's own activation rules, which is what lets Enter and Space both reach it.
/// Its label carries the call and its description the status, so a reader moving by controls
/// hears both without reading the output.
pub(super) fn tool_card<S: CardPress>(index: usize, id: &str, call: &Card<'_>, kit: &Kit) -> El {
    let Card {
        name,
        status,
        output,
        preview,
        target,
        expanded,
    } = *call;
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
        .child(header::<S>(index, id, name, target, status, expanded, kit));
    if let Some(preview) = preview {
        card = card.child(preview_view(preview, kit));
    }
    if output.trim().is_empty() {
        // A card with nothing to read says so, and the keys above leave it shut: an opened block
        // of no lines would read as a call that answered with nothing rather than one that did
        // not answer.
        card = card.child(
            text(status.empty_words())
                .key(format!("tool-{index}-empty"))
                .font(kit.ty.small)
                .color(kit.c.text_faint),
        );
        return card;
    }
    let recorded = recorded(output);
    if expanded {
        card = card.child(recording(index, &recorded, kit));
        card = card.child(
            text(opened_output(output, recorded.bytes))
                .key(format!("tool-{index}-output"))
                .font(kit.ty.mono)
                .color(kit.c.text_base)
                .selectable(true),
        );
    } else {
        card = card.child(
            text(collapsed_output(output))
                .key(format!("tool-{index}-output"))
                .font(kit.ty.mono)
                .color(kit.c.text_muted)
                .selectable(true),
        );
    }
    card
}

/// The card's first line — the tool, what it acted on, and how it ended — which is also the press
/// that opens the output.
fn header<S: CardPress>(
    index: usize,
    id: &str,
    name: &str,
    target: Option<&str>,
    status: ToolStatus,
    expanded: bool,
    kit: &Kit,
) -> El {
    let mut head = row().gap(8.0).center_y().child(
        text(name.to_string())
            .font(kit.ty.small_medium)
            .color(kit.c.text_base),
    );
    if let Some(target) = target {
        head = head.child(
            text(target.to_string())
                .font(kit.ty.mono)
                .color(kit.c.text_muted)
                .truncate_middle(),
        );
    }
    let id = id.to_string();
    row()
        .key(format!("tool-{index}-head"))
        .role("button")
        .label(if expanded {
            format!("Hide the output of {name}")
        } else {
            format!("Show the output of {name}")
        })
        .description(status.words())
        .on_press(move |state: &mut S| state.card_press(index, &id))
        .w_full()
        .justify_between()
        .gap(12.0)
        .child(head)
        .child(
            text(status.words())
                .font(kit.ty.small)
                .color(status_ink(status, kit)),
        )
}

/// The line over an opened block: how much the transcript holds, and whether the tool had already
/// cut it when it was recorded.
fn recording(index: usize, recorded: &Recorded, kit: &Kit) -> El {
    let mut line = col()
        .key(format!("tool-{index}-recording"))
        .w_full()
        .gap(2.0)
        .child(
            text(recorded.words())
                .font(kit.ty.small)
                .color(kit.c.text_faint),
        );
    if let Some(cut) = recorded.cut_words() {
        line = line.child(
            text(cut)
                .key(format!("tool-{index}-cut"))
                .font(kit.ty.small)
                .color(kit.c.warning.fg),
        );
    }
    line
}
