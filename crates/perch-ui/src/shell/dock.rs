//! The terminal dock: the frame's runs as the grid a shell wrote.
//!
//! The header says what the tab is doing, and — when the grid is above the live bottom — where it
//! is instead. The grid itself takes the pointer and the keys: a wheel or Shift+PageUp moves the
//! history, a drag selects cells and copies them, and a press with no drag clears the selection.

mod input;
mod row;
mod words;

#[cfg(test)]
mod budget;
#[cfg(test)]
mod geometry;
#[cfg(test)]
mod motion;
#[cfg(test)]
mod tests;

use perch_term::Frame;
use rune::{Cx, El, ScrollSpec, Tx, WheelEvent, col, row, text};
use rune_kit::Kit;
use rune_kit::shell::{TabItem, TabOrientation, TabStrip};

use super::live::Live;
use super::paint::cursor_cell;
use super::pump::grid_for;
use super::state::Shell;
use super::terminal::{TerminalTab, scroll_words};
use super::{DOCK, TABS};
use crate::files::section_label;
use crate::terminal_view::{RowRun, paint_rows};
use row::{row_height, rows_note, terminal_row};

pub use input::terminal_key;

/// What the dock says when there is no tab at all.
const STOPPED: &str = "stopped";

/// The terminal dock, under the transcript and no wider than it.
pub(super) fn dock<L: Live>(shell: &Shell<L>, kit: &Kit, cx: &mut Cx<Shell<L>>) -> El {
    // The grid the pty is told about follows what the dock drew last frame, so a resized window
    // reaches the shell's own line wrapping. The same count is how many lines the grid paints: a
    // box that is not a whole number of lines holds only the whole ones, and the box is then cut
    // to them, so what is drawn and what the pty was told are one number.
    let rows = cx.layout_size("terminal-rows").map(|[width, height]| {
        shell.measure(width, height, kit);
        grid_for(width, height, kit.ty.mono).1
    });
    let tab = shell.active_terminal();
    let (title, state) = match tab {
        Some(tab) => (tab.title().to_owned(), tab.state()),
        None => (String::new(), STOPPED),
    };
    let mut status = if title.is_empty() {
        state.to_owned()
    } else {
        format!("{title} · {state}")
    };
    // A grid that is a page into the history says where it is: a reader who scrolled back must be
    // able to tell the frozen screen from a shell that stopped printing.
    if let Some(words) = tab.and_then(|tab| scroll_words(tab.offset())) {
        status = format!("{status} · {words}");
    }
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
    panel.child(dock_body::<L>(tab, kit, rows))
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
///
/// `rows` is the count the pty was told the dock holds, so it is also how many lines are painted
/// and the one number the grid's height is built from. A frame that reports fewer rows than that
/// is one the pty has not reflowed yet; one that reports more paints only what fits.
fn dock_body<L: Live>(tab: Option<&TerminalTab>, kit: &Kit, rows: Option<u16>) -> El {
    let line = row_height(kit);
    // A shell that has exited says so. Its last frame is not painted: a frozen grid reads as a
    // shell that is still printing, and the one thing the window must not do is pretend.
    let Some(tab) = tab else {
        return rows_note(kit, "no shell running");
    };
    if let Some(ended) = tab.ended() {
        return rows_note(kit, ended);
    }
    let Some(frame) = tab.frame() else {
        return rows_note(kit, "no shell running");
    };
    // Follow the bottom, the way a terminal does: the newest output is the row in view. The grid
    // takes keys, so a click into it types at the shell, and it takes the wheel so a terminal's
    // own history moves instead of the panel's scroll bar.
    let cursor = cursor_cell(frame);
    let selection = tab.selection();
    let cells = rows_of(frame);
    let lines = rows.map_or(cells.len(), usize::from);
    let mut grid = col()
        .key("terminal-rows")
        .role("grid")
        .label("Terminal output")
        // A terminal is a grid of text, not a layout: the caret moves one cell when a character
        // echoes, and every change inside this subtree is a new picture of the same screen. Rune
        // animates a commit by default, so without this the cursor slides to meet the typing;
        // landing it here covers every commit that can move it — the poll that installs a frame
        // as much as a key press, which is the one a handler's `tx.instant()` cannot reach.
        .instant_changes()
        // A reader cannot see the block or the wash, so both are said here, once, on the grid
        // that holds them.
        .description(words::grid(frame, selection))
        .w_full()
        // The grid is exactly the lines it holds. Left to grow into the dock's box it would be
        // taller than the rows painted in it, and that remainder is the strip under the last line.
        .h(lines as f32 * line)
        .min_h(0.0)
        .gap(0.0)
        .focusable(true)
        .on_key(input::terminal_key::<L>)
        .on_wheel(move |shell: &mut Shell<L>, ev: WheelEvent, tx: &mut Tx| {
            input::terminal_wheel(shell, ev, tx, line)
        })
        .on_drag(input::terminal_drag::<L>)
        .scroll(ScrollSpec { stick_bottom: true });
    for (index, runs) in cells.iter().take(lines).enumerate() {
        let Ok(index) = u16::try_from(index) else {
            break;
        };
        grid = grid.child(terminal_row(index, runs, frame, cursor, selection, kit));
    }
    grid
}

/// The runs of every row of `frame`, one group per row.
///
/// A frame is the whole visible grid, so every row of it is a line of the dock: a row with nothing
/// on it is a line the shell left blank, and one that is dropped leaves the rows below it unpainted
/// and the box taller than what is drawn in it.
fn rows_of(frame: &Frame) -> Vec<Vec<RowRun>> {
    let mut cells: Vec<Vec<RowRun>> = vec![Vec::new(); frame.rows.len()];
    for run in paint_rows(frame) {
        if let Some(row) = cells.get_mut(usize::from(run.row)) {
            row.push(run);
        }
    }
    cells
}
