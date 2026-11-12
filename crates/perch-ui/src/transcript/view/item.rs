//! One item's node: what a turn, a tool card or a system line draws.

use rune::{El, col, text};
use rune_kit::Kit;
use rune_kit::markdown::Markdown;

use super::speaker;
use crate::transcript::card::{Card, tool_card};
use crate::transcript::words::summary_line;
use crate::transcript::{Item, Transcript};

/// Where the turn that a node belongs to got to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TurnState {
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

/// One item, keyed by its place in the column so a growing transcript keeps earlier elements.
///
/// A found item is wrapped in one element of its own, keyed `find-hit-<index>`: that wrapper is
/// what the search scrolls to and what a reader sees marked, because a run of text inside a
/// rendered markdown child is not reachable from this crate.
pub(super) fn item_view(
    index: usize,
    item: &Item,
    state: TurnState,
    model: &Transcript,
    kit: &Kit,
    found: bool,
) -> El {
    let view = match item {
        Item::User { text: body } => col()
            .key(item.key(index))
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
                .key(item.key(index))
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
        // Thinking is spoken in the window's own quiet voice: a mark in the margin, the face the
        // column uses for asides, and plain text, because a model's reasoning is not the reply and
        // must never look like one.
        Item::Reasoning { text: body } => col()
            .key(item.key(index))
            .role("group")
            .label("Thinking")
            .w_full()
            .gap(3.0)
            .px(8.0)
            .child(
                text("Thinking")
                    .font(kit.ty.small_medium)
                    .color(kit.c.text_faint),
            )
            .child(
                text(body.clone())
                    .key("reasoning-body")
                    .w_full()
                    .font(kit.ty.small)
                    .color(kit.c.text_muted)
                    .selectable(true),
            ),
        Item::Tool {
            id,
            name,
            status,
            output,
            preview,
            target,
        } => tool_card(
            index,
            &Card {
                name,
                status: *status,
                output,
                preview: preview.as_ref(),
                target: target.as_deref(),
                expanded: model.is_expanded(id),
            },
            kit,
        ),
        // A system line is drawn as it is written: one text element, no markdown, in the faint
        // ink a footnote uses, so it never competes with what the model said.
        Item::Note(body) => text(body.clone())
            .key(item.key(index))
            .w_full()
            .font(kit.ty.small)
            .color(kit.c.text_faint)
            .selectable(true),
        Item::Summary {
            tools,
            failed,
            tokens,
        } => text(summary_line(*tools, *failed, *tokens))
            .key(item.key(index))
            .w_full()
            .font(kit.ty.small)
            .color(kit.c.text_faint)
            .selectable(true),
    };
    if !found {
        return view;
    }
    col()
        .key(format!("find-hit-{index}"))
        .role("group")
        .label("Found text")
        .w_full()
        .border(1.0, kit.c.border_strong)
        .radius(kit.radius.md)
        .child(view)
}
