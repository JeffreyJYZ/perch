//! The window's regions, composed: the left column, the centre, and the dock under it.

use rune::{Cx, El, col, el, row};
use rune_kit::Kit;
use rune_kit::glyphs;
use rune_kit::shell::{TabItem, TabOrientation, TabStrip};

use super::composer::composer;
use super::dock::dock;
use super::live::Live;
use super::sidebar::{files, sessions};
use super::state::Shell;
use super::{COLUMN, TABS};
use crate::transcript::transcript_view;

/// The whole window.
pub fn view<L: Live>(shell: &Shell<L>, cx: &mut Cx<Shell<L>>) -> El {
    let kit = Kit::neutral(cx.system_dark());
    row()
        .key("perch")
        .w_full()
        .h_full()
        .fill(kit.c.bg_deep)
        .child(column(shell, &kit))
        .child(rule(&kit, "column-rule").w(1.0).h_full())
        .child(workspace(shell, &kit, cx))
}

/// A hairline that separates two regions; the caller gives it its one-pixel axis. Decorative: the
/// regions already name themselves.
fn rule(kit: &Kit, key: &str) -> El {
    el().key(key).fill(kit.c.border_base).a11y_hidden()
}

/// The left column, full window height: the session list above the file tree.
fn column<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    col()
        .key("left-column")
        .role("region")
        .label("Sidebar")
        .w(COLUMN)
        .h_full()
        .min_h(0.0)
        .fill(kit.c.bg_base)
        .child(sessions(shell, kit))
        .child(files(shell, kit))
}

/// The centre: the tab strip, the transcript with the prompt under it, and the dock under the
/// transcript alone.
fn workspace<L: Live>(shell: &Shell<L>, kit: &Kit, cx: &mut Cx<Shell<L>>) -> El {
    col()
        .key("workspace")
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .fill(kit.c.bg_deep)
        .child(tabs(shell, kit))
        .child(centre(shell, kit))
        .child(rule(kit, "dock-rule").w_full().h(1.0))
        .child(dock(shell, kit, cx))
}

/// The session tabs. Selecting one is the same state swap the rows make.
///
/// The strip grows along its parent's main axis, which is right in a titlebar and wrong in a
/// column, so it gets a row of its own: one tab row's height, the transcript takes the rest.
fn tabs<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    let items: Vec<TabItem<'_, Shell<L>>> = shell
        .sessions
        .iter()
        .map(|session| {
            TabItem::new(session.id.clone(), session.title.clone())
                .icon(&glyphs::CURSOR_TEXT)
                .closable(false)
        })
        .collect();
    let active = shell.active_session().map(|session| session.id.as_str());
    let strip = TabStrip::new("session-tabs", TabOrientation::Horizontal)
        .label("Session tabs")
        .tabs(items)
        .active(active)
        .on_select(|shell: &mut Shell<L>, id: &str, _: &mut rune::Tx| {
            if let Some(index) = shell.sessions.iter().position(|session| session.id == id) {
                shell.select(index);
            }
        })
        .build(kit);
    row().key("tab-row").w_full().h(TABS).child(strip)
}

/// The transcript column: the turns, and the prompt at the bottom of it.
///
/// The prompt sits in this column rather than under the dock, so the dock's bottom edge is the
/// window's and the composer belongs to what it asks.
fn centre<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    col()
        .key("transcript-column")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .child(transcript_view(&shell.transcript))
        .child(composer(shell, kit))
}
