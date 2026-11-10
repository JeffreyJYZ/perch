//! The window: one left column of sessions over files, the transcript in the centre, and the
//! terminal dock under the transcript only — so the dock belongs to the session it shows.
//!
//! Every state the layout shows is a word ("waiting", "done", "failed", "approval needed");
//! colour never carries meaning on its own.

use perch_store::SessionMeta;
use rune::{Cx, El, Tx, col, el, row, text};
use rune_kit::Kit;
use rune_kit::glyphs;
use rune_kit::shell::{SidebarRow, TabItem, TabOrientation, TabStrip};

/// The left column's width, in points.
const COLUMN: f32 = 276.0;
/// The terminal dock's height, in points.
const DOCK: f32 = 220.0;
/// The tab strip's row height, in points.
const TABS: f32 = 38.0;
/// How tall one session row is, in points.
const ROW: f32 = 40.0;

/// The sessions the window shows and which one is active.
///
/// A session switch is a state swap: the list, the tabs, the transcript and the terminal all
/// read from this one index, and nothing is reloaded from disk.
pub struct Shell {
    sessions: Vec<SessionMeta>,
    active: usize,
}

impl Shell {
    /// The shell over `sessions`, with `active` selected. An out-of-range `active` falls back to
    /// the first session, so a caller never has to clamp it.
    pub fn new(sessions: Vec<SessionMeta>, active: usize) -> Shell {
        let active = if sessions.is_empty() {
            0
        } else {
            active.min(sessions.len() - 1)
        };
        Shell { sessions, active }
    }

    /// The active session's index.
    pub fn active(&self) -> usize {
        self.active
    }

    /// Selects the session at `index`. An index past the end is ignored rather than clamped: it
    /// names a session that is gone, and switching to a neighbour would switch to the wrong one.
    pub fn select(&mut self, index: usize) {
        if index < self.sessions.len() {
            self.active = index;
        }
    }

    /// Every session's title, in tab order.
    pub fn tab_titles(&self) -> Vec<String> {
        self.sessions
            .iter()
            .map(|session| session.title.clone())
            .collect()
    }

    /// The active session, when there is one.
    pub fn active_session(&self) -> Option<&SessionMeta> {
        self.sessions.get(self.active)
    }
}

/// The whole window.
pub fn view(shell: &Shell, cx: &mut Cx<Shell>) -> El {
    let kit = Kit::neutral(cx.system_dark());
    row()
        .key("perch")
        .w_full()
        .h_full()
        .fill(kit.c.bg_deep)
        .child(column(shell, &kit))
        .child(rule(&kit, "column-rule").w(1.0).h_full())
        .child(workspace(shell, &kit))
}

/// A hairline that separates two regions; the caller gives it its one-pixel axis. Decorative:
/// the regions already name themselves.
fn rule(kit: &Kit, key: &str) -> El {
    el().key(key).fill(kit.c.border_base).a11y_hidden()
}

/// The left column, full window height: the session list above the file tree.
fn column(shell: &Shell, kit: &Kit) -> El {
    col()
        .key("left-column")
        .w(COLUMN)
        .h_full()
        .min_h(0.0)
        .fill(kit.c.bg_base)
        .child(sessions(shell, kit))
        .child(files(kit))
}

/// The session list, and the tab strip it is the long form of.
fn sessions(shell: &Shell, kit: &Kit) -> El {
    let mut list = col()
        .key("sessions")
        .role("region")
        .label("Sessions")
        .w_full()
        .shrink()
        .px(10.0)
        .pt(12.0)
        .pb(8.0)
        .gap(2.0)
        .child(
            text("Sessions")
                .font(kit.ty.small_medium)
                .color(kit.c.text_faint),
        );
    if shell.sessions.is_empty() {
        list = list.child(
            text("No sessions yet")
                .font(kit.ty.small)
                .color(kit.c.text_faint),
        );
    }
    for (index, session) in shell.sessions.iter().enumerate() {
        list = list.child(
            SidebarRow::new(session.title.clone())
                .height(ROW)
                .subtitle(describe(session))
                .selected(index == shell.active)
                .on_select(move |s: &mut Shell, _: &mut Tx| s.select(index))
                .build(kit)
                .key(format!("session-{index}")),
        );
    }
    list
}

/// The file tree's slot. The worktree walk lands here; until then it says so in words.
fn files(kit: &Kit) -> El {
    col()
        .key("files")
        .role("region")
        .label("Files")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .px(10.0)
        .pt(10.0)
        .pb(10.0)
        .gap(4.0)
        .child(
            text("Files")
                .font(kit.ty.small_medium)
                .color(kit.c.text_faint),
        )
        .child(
            text("not loaded")
                .font(kit.ty.small)
                .color(kit.c.text_faint),
        )
}

/// The centre: the tab strip, the transcript, and the dock under the transcript alone.
fn workspace(shell: &Shell, kit: &Kit) -> El {
    col()
        .key("workspace")
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .fill(kit.c.bg_deep)
        .child(tabs(shell, kit))
        .child(transcript(shell, kit))
        .child(rule(kit, "dock-rule").w_full().h(1.0))
        .child(dock(kit))
}

/// The session tabs. Selecting one is the same state swap the rows make.
///
/// The strip grows along its parent's main axis, which is right in a titlebar and wrong in a
/// column, so it gets a row of its own: one tab row's height, the transcript takes the rest.
fn tabs(shell: &Shell, kit: &Kit) -> El {
    let items: Vec<TabItem<'_, Shell>> = shell
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
        .on_select(|shell: &mut Shell, id: &str, _: &mut Tx| {
            if let Some(index) = shell.sessions.iter().position(|session| session.id == id) {
                shell.select(index);
            }
        })
        .build(kit);
    row().key("tab-row").w_full().h(TABS).child(strip)
}

/// The transcript column. Turns stream in here; the empty state says so rather than showing
/// nothing.
fn transcript(shell: &Shell, kit: &Kit) -> El {
    let where_ = shell.active_session().map_or_else(
        || "No session selected".to_string(),
        |session| session.worktree.display().to_string(),
    );
    col()
        .key("transcript")
        .role("region")
        .label("Transcript")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .px(20.0)
        .py(16.0)
        .gap(6.0)
        .child(text(where_).font(kit.ty.mono).color(kit.c.text_faint))
        .child(
            text("No turns yet.")
                .font(kit.ty.body)
                .color(kit.c.text_muted),
        )
}

/// The terminal dock, under the transcript and no wider than it.
fn dock(kit: &Kit) -> El {
    col()
        .key("terminal")
        .role("region")
        .label("Terminal")
        .w_full()
        .h(DOCK)
        .fill(kit.c.bg_base)
        .px(20.0)
        .py(12.0)
        .gap(6.0)
        .child(
            row()
                .w_full()
                .justify_between()
                .child(
                    text("Terminal")
                        .font(kit.ty.small_medium)
                        .color(kit.c.text_faint),
                )
                .child(text("stopped").font(kit.ty.small).color(kit.c.text_faint)),
        )
        .child(
            text("no shell running")
                .font(kit.ty.mono)
                .color(kit.c.text_muted),
        )
}

/// A session row's second line: its branch and its worktree, both as words.
fn describe(session: &SessionMeta) -> String {
    let branch = if session.branch.is_empty() {
        "no branch".to_string()
    } else {
        session.branch.clone()
    };
    let worktree = session.worktree.file_name().map_or_else(
        || session.worktree.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    format!("{branch} · {worktree}")
}
