//! The column's drawing: the header that says where the column is scrolled to, the viewport
//! that takes the wheel, and one keyed node per turn.
//!
//! Every turn is a node of its own, named for its speaker, and a reply still arriving says
//! "streaming" where a cancelled one says "cancelled". A reader can move between turns instead of
//! reading every line.

use rune::{Align, El, ScrollSpec, col, row, text};
use rune_kit::Kit;

use super::Transcript;

use item::{TurnState, item_view};

mod item;

/// The width the header's scroll words lay out in. A fixed box, because the text is latched
/// after layout has run and an empty author's box would clip what arrives.
const WORDS_WIDTH: f32 = 160.0;

/// The transcript column: one item after another, each naming what it is.
///
/// The header and the turns scroll together inside one viewport that follows the newest turn:
/// scrolled up it stays where the reader left it, and back at the bottom it follows again.
#[must_use]
pub fn transcript_view(model: &Transcript) -> El {
    transcript_view_marked(model, None)
}

/// The transcript column with item `marked` drawn as the one a search found.
///
/// The mark is the item itself, ringed: marking the matched run inside a rendered markdown child
/// is not reachable from this crate, so the honest smaller thing is done — the item that holds
/// the text is what a reader is taken to and what is outlined.
#[must_use]
pub fn transcript_view_marked(model: &Transcript, marked: Option<usize>) -> El {
    let kit = Kit::neutral(model.dark);
    col()
        .key("transcript")
        .role("region")
        .label("Transcript")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .child(header(model, &kit))
        .child(viewport(model, &kit, marked))
}

/// The line over the column: what it is, and how far above the newest turn it is scrolled.
fn header(model: &Transcript, kit: &Kit) -> El {
    row()
        .key("transcript-header")
        .w_full()
        .justify_between()
        .px(20.0)
        .pt(14.0)
        .pb(4.0)
        .child(crate::files::section_label(
            kit,
            "Transcript",
            "transcript-label",
        ))
        .child(
            text("")
                .key("transcript-scroll-words")
                .live_text(model.scroll().text())
                .w(WORDS_WIDTH)
                .align(Align::End)
                .font(kit.ty.small)
                .color(kit.c.text_faint),
        )
}

/// The turns, in one scroll viewport that takes the wheel and follows the newest turn.
fn viewport(model: &Transcript, kit: &Kit, marked: Option<usize>) -> El {
    let words = model.scroll().clone();
    let line = kit.ty.body.line_px();
    let mut column = col()
        .key("transcript-scroll")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .px(20.0)
        .py(12.0)
        .gap(10.0)
        // One selection across the plain turns of the column, so a drag copies turns, not one
        // turn. A markdown reply brings its own group with it: see `transcript_view`'s docs.
        .selection_group(true)
        .scroll(ScrollSpec { stick_bottom: true })
        .on_scroll_offset(move |at| words.track(at, line));
    if model.items().is_empty() {
        column = column.child(
            text("No turns yet.")
                .font(kit.ty.body)
                .color(kit.c.text_muted),
        );
    }
    for (index, item) in model.items().iter().enumerate() {
        let state = if model.is_open(index) {
            TurnState::Streaming
        } else if model.is_cancelled(index) {
            TurnState::Cancelled
        } else {
            TurnState::Settled
        };
        column = column.child(item_view(
            index,
            item,
            state,
            model,
            kit,
            marked == Some(index),
        ));
    }
    column
}

/// The small label above a turn's or a card's body.
pub(super) fn speaker(label: &str, kit: &Kit) -> El {
    text(label)
        .font(kit.ty.small_medium)
        .color(kit.c.text_faint)
}
