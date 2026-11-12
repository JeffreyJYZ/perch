//! ⌘F's bar: the field, where it is in the hits, and the controls that move or close it.
//!
//! The bar is drawn inside the transcript column rather than as an overlay, so the column keeps
//! its scroll and its selection while the reader searches. Every press lands in
//! [`Shell::find_key`](super::search_transcript::Find) and the state decides what it means, so the
//! keyboard and the pointer run the same path.

use rune::{El, KeyEvent, Tx, row, text};
use rune_kit::glyphs;
use rune_kit::{IconButton, IconButtonVariant, Kit, TextInput};

use super::live::Live;
use super::state::Shell;

/// How wide the query field is, in points. A fixed width keeps the count and the buttons beside
/// it wherever the pointer lands.
const FIELD: f32 = 220.0;

/// The bar, when ⌘F opened it: a full-width row over the turns.
pub(super) fn bar<L: Live>(shell: &Shell<L>, kit: &Kit) -> Option<El> {
    let find = shell.find.as_ref()?;
    Some(
        row()
            .key("find-bar")
            .role("region")
            .label("Search the transcript")
            .w_full()
            .center_y()
            .gap(8.0)
            .px(20.0)
            .py(6.0)
            .fill(kit.c.bg_base)
            .child(
                TextInput::new(&find.query)
                    .label("Search the transcript")
                    .placeholder("Find in the conversation")
                    .width(Some(FIELD))
                    .on_edit(|shell: &mut Shell<L>| {
                        &mut shell.find.as_mut().expect("the bar is open").query
                    })
                    .on_key(|shell: &mut Shell<L>, ev: &KeyEvent, tx: &mut Tx| {
                        shell.find_key(ev, tx)
                    })
                    .on_change(|shell: &mut Shell<L>, tx: &mut Tx| shell.find_changed(tx))
                    .build(kit)
                    .key("find-query"),
            )
            .child(
                text(shell.find_words())
                    .key("find-count")
                    .grow(1.0)
                    .font(kit.ty.small)
                    .color(kit.c.text_muted),
            )
            .child(step::<L>(
                -1,
                &glyphs::CHEVRON_UP,
                "Previous match",
                "find-prev",
                kit,
            ))
            .child(step::<L>(
                1,
                &glyphs::CHEVRON_DOWN,
                "Next match",
                "find-next",
                kit,
            ))
            .child(
                IconButton::new(&glyphs::CLOSE)
                    .variant(IconButtonVariant::Ghost)
                    .label("Close the search")
                    .on_press(|shell: &mut Shell<L>| shell.close_find())
                    .build(kit)
                    .key("find-close"),
            ),
    )
}

/// One step control: pressed, it moves the pick and takes the column to the hit.
fn step<L: Live>(
    direction: isize,
    icon: &'static rune::Icon,
    label: &str,
    key: &str,
    kit: &Kit,
) -> El {
    IconButton::new(icon)
        .variant(IconButtonVariant::Ghost)
        .label(label)
        .on_press_tx(move |shell: &mut Shell<L>, tx: &mut Tx| {
            shell.find_step(direction);
            tx.instant();
            shell.find_scroll(tx);
        })
        .build(kit)
        .key(key)
}
