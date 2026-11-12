//! The autocomplete list above the composer: one row per command the typed prefix keeps.
//!
//! It is drawn only while the list is open, so an ordinary prompt is unchanged. A press names the
//! row it hit rather than a position, so the same list takes rows by pointer and by arrow alike.

use rune::{El, Tx, col, text};
use rune_kit::Kit;
use rune_kit::shell::SidebarRow;

use super::live::Live;
use super::state::Shell;

/// How tall one row of the list is, in points: one line, so several fit above the prompt.
const ROW: f32 = 26.0;

/// The list, when the composer has a command being typed in it.
pub(super) fn menu<L: Live>(shell: &Shell<L>, kit: &Kit) -> Option<El> {
    let rows = shell.command_menu()?;
    let picked = shell.menu_index();
    let mut list = col()
        .key("command-menu")
        .role("listbox")
        .label("Commands")
        .w_full()
        .gap(1.0)
        .fill(kit.c.layer_01)
        .radius(kit.radius.md)
        .py(4.0);
    if rows.is_empty() {
        return Some(
            list.child(
                text("no command starts with that")
                    .font(kit.ty.small)
                    .color(kit.c.text_muted),
            ),
        );
    }
    for (index, command) in rows.iter().enumerate() {
        list = list.child(
            SidebarRow::new(command.label())
                .height(ROW)
                .detail(command.about)
                .selected(index == picked)
                .select_only()
                .on_select(move |shell: &mut Shell<L>, _: &mut Tx| {
                    shell.command_menu_pick(index);
                    shell.accept_menu();
                })
                .build(kit)
                .key(format!("command-{index}")),
        );
    }
    Some(list)
}
