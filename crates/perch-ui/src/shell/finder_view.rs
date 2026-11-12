//! The finder's elements: a scrim, the field, and one row per file it kept.
//!
//! Nothing here decides anything: a press lands in [`Shell::finder_key`] or [`Shell::finder_take`]
//! and the state owns what it means, so the keyboard and the pointer run the same path.

use rune::{El, KeyEvent, Tx, col, el, row, text};
use rune_kit::glyphs;
use rune_kit::shell::SidebarRow;
use rune_kit::{IconButton, IconButtonVariant, Kit, TextInput};

use super::ROW;
use super::finder::{Finder, FinderRow, MAX_FILES, MAX_ROWS};
use super::live::Live;
use super::state::Shell;
use crate::files::{note, section_label};

/// The overlay, when the finder is open: a full-window scrim and the panel over it.
///
/// It is a layer over the whole window rather than a pane, so Escape returns to whatever was
/// showing — a file's preview stays the pane, and its tab stays in the strip.
pub(super) fn layer<L: Live>(shell: &Shell<L>, kit: &Kit) -> Option<El> {
    let finder = shell.finder.as_ref()?;
    let rows = finder.rows();
    let truncated = finder.truncated;
    let mut body = col()
        .key("finder-panel")
        .role("dialog")
        .label("Find a file")
        .w(640.0)
        .gap(10.0)
        .px(16.0)
        .py(14.0)
        .radius(kit.radius.lg)
        .fill(kit.c.bg_base)
        .border(1.0, kit.c.border_base)
        .child(header::<L>(kit))
        .child(
            TextInput::new(&finder.query)
                .label("Find a file")
                .placeholder("type part of a path")
                .width(None)
                .on_edit(|shell: &mut Shell<L>| {
                    &mut shell.finder.as_mut().expect("the finder is open").query
                })
                .on_key(|shell: &mut Shell<L>, ev: &KeyEvent, _: &mut Tx| shell.finder_key(ev))
                .on_submit(|shell: &mut Shell<L>, _: &mut Tx| {
                    shell.finder_take();
                })
                .build(kit)
                .key("finder-query"),
        )
        .child(section_label(kit, "Files", "finder-label"));
    body = match empty(shell, finder, &rows) {
        Some(words) => body.child(note(kit, words)),
        None => body.child(list(shell, kit, &rows)),
    };
    if shell.finder_total() > rows.len() {
        body = body.child(note(
            kit,
            &format!(
                "{} matches: the first {MAX_ROWS} are drawn",
                shell.finder_total()
            ),
        ));
    }
    if truncated {
        body = body.child(note(
            kit,
            &format!("stopped after {MAX_FILES} files, so some are not listed"),
        ));
    }
    Some(
        el().key("finder")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .child(rune_kit::scrim("finder-scrim", |shell: &mut Shell<L>| {
                shell.close_finder();
            }))
            .child(
                row()
                    .key("finder-anchor")
                    .absolute(0.0, 0.0)
                    .w_full()
                    .h_full()
                    .pt(72.0)
                    .px(24.0)
                    .child(rune::spacer())
                    .child(body.modal(true, |shell: &mut Shell<L>| shell.close_finder()))
                    .child(rune::spacer()),
            ),
    )
}

/// Why the list is empty, in words, or `None` when there is a list to draw.
fn empty<L: Live>(shell: &Shell<L>, finder: &Finder, rows: &[FinderRow]) -> Option<&'static str> {
    if shell.workspace().is_none() {
        return Some("no folder is open, so there is nothing to find");
    }
    if finder.files.is_empty() {
        return Some("this worktree has no files to find");
    }
    if rows.is_empty() {
        return Some("no file matches what is typed");
    }
    None
}

/// One row per kept file: its name, the folder above it, and its press opening that file.
fn list<L: Live>(shell: &Shell<L>, kit: &Kit, rows: &[FinderRow]) -> El {
    let picked = shell.finder_index();
    let mut list = col().key("finder-rows").w_full().gap(2.0);
    for (index, entry) in rows.iter().enumerate() {
        list = list.child(
            SidebarRow::new(entry.name.clone())
                .height(ROW)
                .subtitle(entry.dir.clone())
                .selected(index == picked)
                .on_select(move |shell: &mut Shell<L>, _: &mut Tx| {
                    shell.finder_pick(index);
                    shell.finder_take();
                })
                .build(kit)
                .key(format!("finder-{index}")),
        );
    }
    list
}

/// The title and the one way out.
fn header<L: Live>(kit: &Kit) -> El {
    row()
        .key("finder-header")
        .w_full()
        .center_y()
        .child(
            text("Find a file")
                .w_full()
                .grow(1.0)
                .font(kit.ty.ui_medium)
                .color(kit.c.text_base),
        )
        .child(
            IconButton::new(&glyphs::CLOSE)
                .variant(IconButtonVariant::Ghost)
                .label("Close the finder")
                .on_press(|shell: &mut Shell<L>| shell.close_finder())
                .build(kit)
                .key("finder-close"),
        )
}
