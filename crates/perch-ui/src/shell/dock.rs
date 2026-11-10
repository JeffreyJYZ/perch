//! The terminal dock: the frame's runs as the grid a shell wrote.

use rune::{Cx, El, KeyEvent, ScrollSpec, Tx, col, row, text};
use rune_kit::Kit;
use rune_kit::shell::{TabItem, TabOrientation, TabStrip};

use super::live::Live;
use super::paint::terminal_text;
use super::state::Shell;
use super::terminal::TerminalTab;
use super::{DOCK, TABS};
use crate::files::{note, section_label};
use crate::terminal_view::{RowRun, paint_rows};

/// What the dock says when there is no tab at all.
const STOPPED: &str = "stopped";

/// The terminal dock, under the transcript and no wider than it.
pub(super) fn dock<L: Live>(shell: &Shell<L>, kit: &Kit, cx: &mut Cx<Shell<L>>) -> El {
    // The grid the pty is told about follows what the dock drew last frame, so a resized window
    // reaches the shell's own line wrapping.
    if let Some([width, height]) = cx.layout_size("terminal-rows") {
        shell.measure(width, height, kit);
    }
    let tab = shell.active_terminal();
    let (title, state) = match tab {
        Some(tab) => (tab.title().to_owned(), tab.state()),
        None => (String::new(), STOPPED),
    };
    let status = if title.is_empty() {
        state.to_owned()
    } else {
        format!("{title} · {state}")
    };
    let mut panel = col()
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
                .child(section_label(kit, "Terminal", "terminal-label"))
                .child(text(status).font(kit.ty.small).color(kit.c.text_faint)),
        );
    // One tab needs no strip: the header already names it. Two or more need one to say which is
    // shown, and to be the thing that switches them.
    if shell.terminals.len() > 1 {
        panel = panel.child(terminal_tabs(shell, kit));
    }
    panel.child(dock_body::<L>(tab, kit))
}

/// The terminal tabs, in strip order, with the shown one selected.
fn terminal_tabs<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    let items: Vec<TabItem<'_, Shell<L>>> = shell
        .terminals
        .iter()
        .enumerate()
        .map(|(index, tab)| TabItem::new(index.to_string(), tab.title().to_owned()).closable(false))
        .collect();
    let active = shell.terminal.to_string();
    let strip = TabStrip::new("terminal-tabs", TabOrientation::Horizontal)
        .label("Terminal tabs")
        .tabs(items)
        .active(Some(active.as_str()))
        .on_select(|shell: &mut Shell<L>, id: &str, _: &mut Tx| {
            if let Ok(index) = id.parse::<usize>() {
                shell.select_terminal(index);
            }
        })
        .build(kit);
    row()
        .key("terminal-tab-row")
        .w_full()
        .h(TABS - 6.0)
        .child(strip)
}

/// The dock's grid: the frame's rows as runs, or the reason there is nothing to draw.
fn dock_body<L: Live>(tab: Option<&TerminalTab>, kit: &Kit) -> El {
    // A shell that has exited says so. Its last frame is not painted: a frozen grid reads as a
    // shell that is still printing, and the one thing the window must not do is pretend.
    if let Some(ended) = tab.and_then(TerminalTab::ended) {
        return col()
            .key("terminal-rows")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .child(note(kit, ended));
    }
    let Some(frame) = tab.and_then(TerminalTab::frame) else {
        return col()
            .key("terminal-rows")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .child(note(kit, "no shell running"));
    };
    // Follow the bottom, the way a terminal does: the newest output is the row in view. The grid
    // takes keys, so a click into it types at the shell.
    let mut grid = col()
        .key("terminal-rows")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(0.0)
        .focusable(true)
        .on_key(terminal_key::<L>)
        .scroll(ScrollSpec { stick_bottom: true });
    // [`paint_rows`] emits runs top to bottom, so a row is one stretch of entries that name the
    // same `row`; each stretch becomes one line of the grid.
    let mut row: Option<u16> = None;
    let mut group: Vec<RowRun> = Vec::new();
    for run in paint_rows(frame) {
        if row != Some(run.row) {
            if let Some(index) = row {
                grid = grid.child(terminal_row(index, &group, kit));
            }
            group.clear();
            row = Some(run.row);
        }
        group.push(run);
    }
    if let Some(index) = row {
        grid = grid.child(terminal_row(index, &group, kit));
    }
    grid
}

/// One row of the grid: its runs side by side, keyed by the row index so a repaint keeps the
/// lines above it.
fn terminal_row(index: u16, runs: &[RowRun], kit: &Kit) -> El {
    let mut line = row().key(format!("terminal-row-{index}")).w_full().gap(0.0);
    for run in runs {
        line = line.child(terminal_text(run, kit));
    }
    line
}

/// Sends one press to the shown terminal tab. True when the shell was handed it.
pub fn terminal_key<L: Live>(shell: &mut Shell<L>, ev: &KeyEvent, _: &mut Tx) -> bool {
    // Cmd chords are the window's own: a terminal never wants ⌘Q as text.
    if ev.mods.cmd {
        return false;
    }
    match shell.terminals.get_mut(shell.terminal) {
        Some(tab) => tab.type_key(ev),
        None => false,
    }
}
