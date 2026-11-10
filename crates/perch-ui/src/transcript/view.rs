//! The column's drawing: the header that says where the column is scrolled to, the viewport
//! that takes the wheel, and one keyed node per turn.
//!
//! Every turn is a node of its own, named for its speaker, and a reply still arriving says
//! "streaming" where a cancelled one says "cancelled". A reader can move between turns instead of
//! reading every line.

use rune::{Align, El, ScrollSpec, col, row, text};
use rune_kit::Kit;
use rune_kit::markdown::Markdown;

use super::card::tool_card;
use super::words::summary_line;
use super::{Item, Transcript};

/// The width the header's scroll words lay out in. A fixed box, because the text is latched
/// after layout has run and an empty author's box would clip what arrives.
const WORDS_WIDTH: f32 = 160.0;

/// Where the turn that a node belongs to got to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TurnState {
    /// The reply is whole.
    Settled,
    /// The turn is still writing into this reply.
    Streaming,
    /// The turn ended before the reply did.
    Cancelled,
}

impl TurnState {
    /// The word the turn's line carries for this state, when it has one.
    fn words(self) -> Option<&'static str> {
        match self {
            TurnState::Settled => None,
            TurnState::Streaming => Some("streaming"),
            TurnState::Cancelled => Some("cancelled"),
        }
    }
}

/// The transcript column: one item after another, each naming what it is.
///
/// The header and the turns scroll together inside one viewport that follows the newest turn:
/// scrolled up it stays where the reader left it, and back at the bottom it follows again.
#[must_use]
pub fn transcript_view(model: &Transcript) -> El {
    let kit = Kit::neutral(model.dark);
    col()
        .key("transcript")
        .role("region")
        .label("Transcript")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .child(header(model, &kit))
        .child(viewport(model, &kit))
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
fn viewport(model: &Transcript, kit: &Kit) -> El {
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
        column = column.child(item_view(index, item, state, model, kit));
    }
    column
}

/// One item, keyed by its place in the column so a growing transcript keeps earlier elements.
fn item_view(index: usize, item: &Item, state: TurnState, model: &Transcript, kit: &Kit) -> El {
    match item {
        Item::User { text: body } => col()
            .key(format!("turn-{index}"))
            .role("group")
            .label("You")
            .w_full()
            .gap(4.0)
            .px(12.0)
            .py(10.0)
            .radius(kit.radius.md)
            .fill(kit.c.layer_01)
            // The group's label already says who spoke, so the drawn line is for the eye only.
            .child(speaker("You", kit).a11y_hidden())
            .child(
                text(body.clone())
                    .key(format!("turn-{index}-text"))
                    .font(kit.ty.body)
                    .color(kit.c.text_base)
                    .selectable(true),
            ),
        Item::Assistant { text: body } => {
            let line = match state.words() {
                Some(word) => format!("Assistant · {word}"),
                None => "Assistant".to_string(),
            };
            let mut turn = col()
                .key(format!("turn-{index}"))
                .role("group")
                .label("Assistant")
                .w_full()
                .gap(4.0);
            if let Some(word) = state.words() {
                turn = turn.description(word);
            }
            turn.child(speaker(&line, kit).a11y_hidden())
                // Streaming mode parses the tail as what it is about to become, so an unclosed
                // fence or emphasis never flashes its markers; a finished turn renders whole.
                .child(
                    Markdown::new(body.clone())
                        .streaming(state == TurnState::Streaming)
                        .selectable(true)
                        .build(kit)
                        .key("body"),
                )
        }
        Item::Tool {
            id,
            name,
            status,
            output,
            preview,
        } => tool_card(
            index,
            name,
            *status,
            output,
            preview.as_ref(),
            model.is_expanded(id),
            kit,
        ),
        // A system line is drawn as it is written: one text element, no markdown, in the faint
        // ink a footnote uses, so it never competes with what the model said.
        Item::Note(body) => text(body.clone())
            .key(format!("note-{index}"))
            .w_full()
            .font(kit.ty.small)
            .color(kit.c.text_faint)
            .selectable(true),
        Item::Summary {
            tools,
            failed,
            tokens,
        } => text(summary_line(*tools, *failed, *tokens))
            .key(format!("summary-{index}"))
            .w_full()
            .font(kit.ty.small)
            .color(kit.c.text_faint)
            .selectable(true),
    }
}

/// The small label above a turn's or a card's body.
pub(super) fn speaker(label: &str, kit: &Kit) -> El {
    text(label)
        .font(kit.ty.small_medium)
        .color(kit.c.text_faint)
}
