//! The one tab strip: the sessions, the files opened under them, and the settings last.
//!
//! The strip is the window's navigation, so every pane the workspace can draw is a tab here and
//! exactly the one showing is marked active. Its presses land in [`Shell::show`] and
//! [`Shell::close_tab`], which own the state: nothing in this file decides what the window shows,
//! which is what keeps the strip and the pane from disagreeing.
//!
//! A tab is drawn with a stroke glyph, or with none at all: an icon that reads as a letter is
//! worse than no icon, because it looks like a typo in the label.

use rune::{Cx, El, Icon, row};
use rune_kit::Kit;
use rune_kit::glyphs;
use rune_kit::shell::{TabItem, TabOrientation, TabStrip};

use super::live::Live;
use super::pane::{Pane, file_name, tab_id};
use super::state::Shell;
use super::{SETTINGS, TABS};

/// A session's glyph: a speech bubble, the shape a conversation takes. Feather's message-square
/// (MIT) drawn at 24 so its stroke stays a stroke at 16 points.
static SESSION: Icon = Icon {
    name: "perch-session",
    view: 24.0,
    body: r#"<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>"#,
};

/// The settings glyph: feather's gear (MIT), drawn at 24 so its teeth stay legible at 16 points.
static GEAR: Icon = Icon {
    name: "perch-settings",
    view: 24.0,
    body: r#"<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>"#,
};

/// The strip, on a row of its own: the strip grows along its parent's main axis, which is right in
/// a titlebar and wrong in a column.
pub(super) fn strip<L: Live>(shell: &Shell<L>, kit: &Kit, cx: &Cx<Shell<L>>) -> El {
    // The file tabs' ids and words are built first, so the strip borrows strings that outlive it.
    let files: Vec<(String, String)> = shell
        .file_tabs()
        .iter()
        .map(|path| (tab_id(path), file_name(path)))
        .collect();
    let mut items: Vec<TabItem<'_, Shell<L>>> = shell
        .sessions
        .iter()
        .map(|session| {
            TabItem::new(session.id.clone(), session.title.clone())
                .icon(&SESSION)
                .closable(false)
        })
        .collect();
    for (id, title) in &files {
        items.push(
            TabItem::new(id.clone(), title.clone())
                .icon(&glyphs::FILE)
                .closable(true),
        );
    }
    items.push(
        TabItem::new(SETTINGS, "Settings")
            .icon(&GEAR)
            .closable(false),
    );
    let active = match shell.pane() {
        Pane::Settings => Some(SETTINGS.to_string()),
        Pane::File(path) => Some(tab_id(path)),
        Pane::Session => shell.active_session().map(|session| session.id.clone()),
    };
    // The tabs share the strip's own width, so the last one keeps its whole label instead of being
    // clipped at the strip's scroll edge. The first frame has no measurement and takes the
    // strip's widest tab.
    let viewport = cx.layout_size("session-tabs").map(|size| size[0]);
    let strip = TabStrip::new("session-tabs", TabOrientation::Horizontal)
        .label("Tabs")
        .tabs(items)
        .viewport_width(viewport)
        .active(active.as_deref())
        .on_select(|shell: &mut Shell<L>, id: &str, _: &mut rune::Tx| shell.show(id))
        .on_close(|shell: &mut Shell<L>, id: &str, _: &mut rune::Tx| {
            shell.close_tab(id);
        })
        .build(kit);
    row().key("tab-row").w_full().h(TABS).child(strip)
}
