//! The window's regions, composed: the left column, the centre, and the dock under it.

use rune::{Cx, El, col, el, row};
use rune_kit::Kit;

use super::COLUMN;
use super::card::card_key;
use super::composer::composer;
use super::dock::dock;
use super::live::Live;
use super::pane::Pane;
use super::panels::{self, Panels};
use super::paste::root_paste;
use super::settings_view;
use super::sidebar::{files, sessions};
use super::state::Shell;
use super::tabs;
use crate::preview::preview_view;
use crate::transcript::transcript_view;

/// The whole window.
///
/// The root is also where two keys arrive that no region owns: a paste, because Rune reads the
/// clipboard at the root only, and a focused card's Enter or Space, because an element's key
/// handlers run before a focused control's own activation rules.
///
/// The plugin column is the last child and only exists when the app has a panel or a word about
/// one, so a window with no plugin is laid out exactly as it was before plugins existed.
pub fn view<L: Live + Panels>(shell: &Shell<L>, cx: &mut Cx<Shell<L>>) -> El {
    let kit = crate::kit::perch(shell.dark());
    let mut root = row()
        .key("perch")
        .w_full()
        .h_full()
        .fill(kit.c.bg_deep)
        .on_key(card_key::<L>)
        .on_paste(root_paste::<L>)
        .child(column(shell, &kit))
        .child(rule(&kit, "column-rule").w(1.0).h_full())
        .child(workspace(shell, &kit, cx));
    if let Some(panels) = panels::column(shell, &kit) {
        root = root
            .child(rule(&kit, "panel-rule").w(1.0).h_full())
            .child(panels);
    }
    root
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

/// The centre: the tab strip, then whatever pane it names, and the dock under both.
///
/// The settings form and a file's preview each take the transcript's place at full width, so the
/// dock keeps its tab and the column it belongs to stays visible beside them. The transcript
/// itself is state, not a drawing: it keeps streaming while another pane is shown.
fn workspace<L: Live>(shell: &Shell<L>, kit: &Kit, cx: &mut Cx<Shell<L>>) -> El {
    let pane = match shell.pane() {
        Pane::Settings => settings_view::panel(shell, kit),
        Pane::File(_) => shell
            .files()
            .map_or_else(|| centre(shell, kit), |panel| preview_view::<L>(panel, kit)),
        Pane::Session => centre(shell, kit),
    };
    col()
        .key("workspace")
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .fill(kit.c.bg_deep)
        .child(tabs::strip(shell, kit, cx))
        .child(pane)
        .child(rule(kit, "dock-rule").w_full().h(1.0))
        .child(dock(shell, kit, cx))
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
