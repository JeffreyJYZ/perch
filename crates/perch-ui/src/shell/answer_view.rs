//! The answer card: what a waiting approval or question expects, and the field a question is
//! answered in.
//!
//! The card is the window's statement of its own keys. Every line names a key and what it does in
//! words, so the card is the reference a person reads once and the screen reader announces, and
//! no line is a bare glyph. The same sentences are what the tests assert, so the card cannot drift
//! from the map that carries the presses out.

use rune::{El, KeyEvent, Tx, col, text};
use rune_kit::Kit;
use rune_kit::TextArea;

use super::keymap::{Typing, interrupts, typing_intent};
use super::live::{Live, Waiting};
use super::state::Shell;

/// The card a turn's wait puts above the prompt. Nothing is waiting, nothing is drawn.
pub(super) fn card<L: Live>(shell: &Shell<L>, kit: &Kit) -> Option<El> {
    let (title, keys): (&'static str, Vec<&'static str>) = match shell.waiting {
        Waiting::Nothing => return None,
        Waiting::Approval => ("Approval", approval_keys().to_vec()),
        Waiting::Question { .. } if shell.typing() => ("Question", typing_keys().to_vec()),
        Waiting::Question { .. } => ("Question", question_keys(shell.waiting).to_vec()),
    };
    let mut card = col()
        .key("answer-card")
        .role("group")
        .label(title)
        .description(keys[0])
        .w_full()
        .gap(4.0)
        .px(12.0)
        .py(8.0)
        .radius(kit.radius.md)
        .fill(kit.c.layer_01)
        .child(text(title).font(kit.ty.small_medium).color(kit.c.text_base));
    for line in keys {
        card = card.child(
            text(line)
                .font(kit.ty.small)
                .color(kit.c.text_muted)
                .selectable(true),
        );
    }
    if shell.typing() {
        card = card.child(field(shell, kit));
    }
    if let Some(words) = picked_words(&shell.answer.picked) {
        card = card.child(text(words).font(kit.ty.small).color(kit.c.text_base));
    }
    if let Some(note) = shell.answer_note() {
        card = card.child(
            text(note.to_string())
                .font(kit.ty.small)
                .color(kit.c.danger.fg),
        );
    }
    Some(card)
}

/// The field a typed answer goes in: its own box, so the composer's draft survives underneath.
fn field<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    TextArea::new(&shell.answer.draft)
        .label("Your answer")
        .placeholder("Type your answer…")
        .rows(2)
        .width(None)
        .on_edit(|shell: &mut Shell<L>| &mut shell.answer.draft)
        .on_key(answer_key::<L>)
        .on_submit(|shell: &mut Shell<L>, _: &mut Tx| {
            shell.send_text();
        })
        .build(kit)
        .key("answer-field")
}

/// The presses the answer field answers: Enter sends, Escape goes back, Ctrl+C stops the turn.
/// Every other press is the field's own, so a digit typed here is a character in the answer.
pub(super) fn answer_key<L: Live>(shell: &mut Shell<L>, ev: &KeyEvent, _: &mut Tx) -> bool {
    // The interrupt still stops the turn, but a bare Escape is this field's way back to the
    // options: treating it as an interrupt would cancel the very question being answered.
    if interrupts(ev) {
        shell.interrupt();
        return true;
    }
    match typing_intent(ev) {
        // Enter and Escape are answered either way: an Enter that sends nothing must not put a
        // newline in the field, and Escape must leave it.
        Some(Typing::Send) => {
            shell.send_text();
            true
        }
        Some(Typing::Back) => {
            shell.cancel_typing();
            true
        }
        None => false,
    }
}

/// The three sentences a question card carries: what picks, what confirms, what types and what
/// cancels.
fn question_keys(waiting: Waiting) -> [&'static str; 3] {
    if waiting.multiple() {
        [
            "Number keys toggle an option on and off.",
            "Enter confirms every option picked.",
            "Press t to type your own answer, Escape to cancel.",
        ]
    } else {
        [
            "Number keys pick an option.",
            "Enter confirms the option picked.",
            "Press t to type your own answer, Escape to cancel.",
        ]
    }
}

/// What an approval card says its three keys do.
fn approval_keys() -> [&'static str; 3] {
    [
        "Enter allows this call once.",
        "Command+Enter allows this tool for the rest of the session.",
        "Escape refuses it, and it does not run.",
    ]
}

/// What the text field says about its own two keys, while it is open.
fn typing_keys() -> [&'static str; 2] {
    [
        "Enter sends what you typed.",
        "Escape goes back to the options without answering.",
    ]
}

/// The options picked, counted from one as the question numbers them, when any are.
fn picked_words(picked: &[usize]) -> Option<String> {
    if picked.is_empty() {
        return None;
    }
    let numbers: Vec<String> = picked.iter().map(|index| (index + 1).to_string()).collect();
    Some(format!("picked: {}", numbers.join(", ")))
}
