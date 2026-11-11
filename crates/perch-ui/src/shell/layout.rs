//! The window's regions, composed: the left column, the centre, and the dock under it.

use rune::{Cx, El, Icon, col, el, row};
use rune_kit::Kit;
use rune_kit::glyphs;
use rune_kit::shell::{TabItem, TabOrientation, TabStrip};
use rune_kit::{IconButton, IconButtonVariant};

use super::card::card_key;
use super::composer::composer;
use super::dock::dock;
use super::live::Live;
use super::panels::{self, Panels};
use super::paste::root_paste;
use super::settings_view;
use super::sidebar::{files, sessions};
use super::state::Shell;
use super::{COLUMN, SETTINGS, TABS};
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
    let kit = Kit::neutral(shell.dark());
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

/// The centre: the tab strip, then whatever takes the transcript's place, and the dock under
/// both.
///
/// The settings panel and a selected file's preview each take the transcript's place at full
/// width, so the dock keeps its tab and the column it belongs to stays visible beside them. The
/// transcript itself is state, not a drawing: it keeps streaming while either one is shown.
fn workspace<L: Live>(shell: &Shell<L>, kit: &Kit, cx: &mut Cx<Shell<L>>) -> El {
    let pane = if shell.settings_open() {
        settings_view::panel(shell, kit)
    } else if let Some(panel) = shell.files().filter(|panel| panel.is_open()) {
        preview_view::<L>(panel)
    } else {
        centre(shell, kit)
    };
    col()
        .key("workspace")
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .fill(kit.c.bg_deep)
        .child(tabs(shell, kit))
        .child(pane)
        .child(rule(kit, "dock-rule").w_full().h(1.0))
        .child(dock(shell, kit, cx))
}

/// The session tabs, the settings tab, and the gear that opens the settings. Selecting a tab is
/// the same state swap the rows make.
///
/// The strip grows along its parent's main axis, which is right in a titlebar and wrong in a
/// column, so it gets a row of its own: one tab row's height, the transcript takes the rest.
fn tabs<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    let mut items: Vec<TabItem<'_, Shell<L>>> = shell
        .sessions
        .iter()
        .map(|session| {
            TabItem::new(session.id.clone(), session.title.clone())
                .icon(&glyphs::CURSOR_TEXT)
                .closable(false)
        })
        .collect();
    items.push(
        TabItem::new(SETTINGS, "Settings")
            .icon(&GEAR)
            .closable(false),
    );
    let active = if shell.settings_open() {
        Some(SETTINGS)
    } else {
        shell.active_session().map(|session| session.id.as_str())
    };
    let strip = TabStrip::new("session-tabs", TabOrientation::Horizontal)
        .label("Tabs")
        .tabs(items)
        .active(active)
        .on_select(|shell: &mut Shell<L>, id: &str, _: &mut rune::Tx| {
            if id == SETTINGS {
                // The settings tab selects the panel, so a press on it while it is open leaves
                // it open rather than flipping it shut.
                if !shell.settings_open() {
                    shell.toggle_settings();
                }
                return;
            }
            shell.close_settings();
            if let Some(index) = shell.sessions.iter().position(|session| session.id == id) {
                shell.select(index);
            }
        })
        .build(kit);
    row()
        .key("tab-row")
        .w_full()
        .h(TABS)
        .child(strip)
        .child(gear::<L>(kit))
}

/// The gear on the tab row: one way into the settings, beside the settings tab and ⌘,.
///
/// Feather's settings glyph (MIT), drawn at 24 so its teeth stay legible at 16 points.
static GEAR: Icon = Icon {
    name: "perch-settings",
    view: 24.0,
    body: r#"<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>"#,
};

/// The gear's press is [`Shell::toggle_settings`], the same action ⌘, takes.
fn gear<L: Live>(kit: &Kit) -> El {
    row().key("settings-slot").pr(8.0).center_y().child(
        IconButton::new(&GEAR)
            .variant(IconButtonVariant::GhostMuted)
            .label("Settings")
            .help("Settings")
            .shortcut("cmd+,")
            .on_press(|shell: &mut Shell<L>| shell.toggle_settings())
            .build(kit)
            .key("settings-gear"),
    )
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
