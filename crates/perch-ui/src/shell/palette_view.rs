//! The palette's elements: the filter field, one row per command or action, and the way out.
//!
//! Nothing here decides anything: a press lands in [`Shell::palette_take`] and the state owns
//! what it means, so the keyboard and the pointer run the same path.

use rune::{El, KeyEvent, ScrollSpec, Tx, col, row, text};
use rune_kit::glyphs;
use rune_kit::shell::SidebarRow;
use rune_kit::{IconButton, IconButtonVariant, Kit, TextInput};

use super::ROW;
use super::live::Live;
use super::state::Shell;
use crate::files::{note, section_label};

/// The palette, in the centre of the window where the transcript was.
pub(super) fn panel<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    let Some(palette) = shell.palette.as_ref() else {
        return col();
    };
    let rows = shell.palette_rows();
    let picked = shell.palette_index();
    let mut body = col()
        .key("palette")
        .role("region")
        .label("Command palette")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .px(24.0)
        .py(20.0)
        .gap(12.0)
        .child(header::<L>(kit))
        .child(
            TextInput::new(&palette.query)
                .label("Search commands")
                .placeholder("type to filter")
                .width(None)
                .on_edit(|shell: &mut Shell<L>| {
                    &mut shell.palette.as_mut().expect("the palette is open").query
                })
                .on_key(|shell: &mut Shell<L>, ev: &KeyEvent, _: &mut Tx| shell.palette_key(ev))
                .on_submit(|shell: &mut Shell<L>, _: &mut Tx| {
                    shell.palette_take();
                })
                .build(kit)
                .key("palette-query"),
        )
        .child(section_label(kit, "Commands", "palette-label"));
    if rows.is_empty() {
        body = body.child(note(kit, "no command matches what was typed"));
    }
    let mut list = col()
        .key("palette-rows")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(2.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        });
    for (index, entry) in rows.iter().enumerate() {
        let label = entry.label();
        let about = entry.about().to_string();
        list = list.child(
            SidebarRow::new(label)
                .height(ROW)
                .subtitle(about)
                .selected(index == picked)
                .on_select(move |shell: &mut Shell<L>, _: &mut Tx| {
                    if let Some(palette) = shell.palette.as_mut() {
                        palette.index = index;
                    }
                    shell.palette_take();
                })
                .build(kit)
                .key(format!("palette-{index}")),
        );
    }
    body.child(list)
}

/// The title and the one way out.
fn header<L: Live>(kit: &Kit) -> El {
    row()
        .key("palette-header")
        .w_full()
        .center_y()
        .child(
            text("Commands")
                .w_full()
                .grow(1.0)
                .font(kit.ty.ui_medium)
                .color(kit.c.text_base),
        )
        .child(
            IconButton::new(&glyphs::CLOSE)
                .variant(IconButtonVariant::Ghost)
                .label("Close the palette")
                .on_press(|shell: &mut Shell<L>| shell.close_palette())
                .build(kit)
                .key("palette-close"),
        )
}
