//! The sidebar's filter field, the count under it, and the session rows it keeps.
//!
//! Nothing here decides anything: the field's words land in the shell's filter and the rows are
//! read back from it, so typing, Escape and a press on a row all run the same path.

use perch_store::SessionMeta;
use rune::{El, KeyEvent, Tx, text};
use rune_kit::glyphs;
use rune_kit::shell::SidebarRow;
use rune_kit::{IconButton, IconButtonVariant, Kit, TextInput};

use super::ROW;
use super::live::Live;
use super::sidebar::describe;
use super::state::Shell;

/// The filter field at the top of the session list.
pub(super) fn filter_field<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    TextInput::new(&shell.session_filter.query)
        .label("Filter sessions")
        .placeholder("Filter sessions…")
        .width(None)
        .on_edit(|shell: &mut Shell<L>| &mut shell.session_filter.query)
        .on_key(|shell: &mut Shell<L>, ev: &KeyEvent, _: &mut Tx| shell.session_filter_key(ev))
        .build(kit)
        .key("session-filter")
}

/// How many rows of how many are drawn, as the words the list shows under its field.
pub(super) fn count<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    text(shell.session_count_words())
        .key("session-count")
        .font(kit.ty.small)
        .color(kit.c.text_faint)
}

/// Why no row is drawn, in the list's own words; `None` when there is a row to draw.
pub(super) fn empty<L: Live>(shell: &Shell<L>) -> Option<String> {
    let filter = shell.session_filter_text().trim();
    if filter.is_empty() || !shell.session_rows().is_empty() {
        return None;
    }
    Some(format!("no session matches \"{filter}\""))
}

/// One session row: its title, its branch and worktree, and its delete control.
pub(super) fn row<L: Live>(shell: &Shell<L>, index: usize, session: &SessionMeta, kit: &Kit) -> El {
    SidebarRow::new(session.title.clone())
        .height(ROW)
        .subtitle(describe(session))
        .trailing(delete_button::<L>(index, kit))
        .selected(index == shell.active)
        .on_select(move |shell: &mut Shell<L>, _: &mut Tx| shell.select(index))
        .build(kit)
        .key(format!("session-{index}"))
}

/// The delete control on a session row, and the words when it cannot run.
///
/// One press arms the row and says so; the second removes it. Deleting a session drops its log,
/// which is the only durable copy of the conversation, so a single stray click must not do it.
fn delete_button<L: Live>(index: usize, kit: &Kit) -> El {
    IconButton::new(&glyphs::CIRCLE_X)
        .variant(IconButtonVariant::Ghost)
        .label(format!("Delete session {index}"))
        .on_press(move |shell: &mut Shell<L>| {
            shell.remove_session(index);
        })
        .build(kit)
        .key(format!("session-delete-{index}"))
}
