//! The flattened tree: one row per node, its icon, its git state and its depth.

use std::path::{Path, PathBuf};

use perch_files::{DIR_ICON, FileStatus, Node, NodeKind, icon_for};
use rune::{El, icon, row, spacer, text};
use rune_kit::Kit;
use rune_kit::glyphs;

use super::status_word;

/// How far one level of depth indents a row, in points.
const INDENT: f32 = 12.0;

/// One line of the flattened tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeRow {
    /// Nesting depth, 0 for a node at the worktree root.
    pub depth: usize,
    /// The row's own name, never its whole path.
    pub name: String,
    /// The Material Symbols name this row's type maps to (`perch_files::icon_for`).
    pub icon: String,
    /// What git reports for the file, `None` for a directory or an unchanged file.
    pub status: Option<FileStatus>,
    /// Whether git reports the file deleted; the row stays visible and cannot be previewed.
    pub deleted: bool,
    /// The row's path, absolute inside the workspace root.
    pub path: PathBuf,
}

/// Every node of `nodes`, in tree order, with the depth each one sits at.
pub(super) fn flatten(nodes: &[Node], depth: usize, out: &mut Vec<TreeRow>) {
    for node in nodes {
        out.push(TreeRow {
            depth,
            name: node.name.clone(),
            icon: match node.kind {
                NodeKind::Dir => DIR_ICON.to_string(),
                NodeKind::File => icon_for(Path::new(&node.name)).to_string(),
            },
            status: node.status,
            deleted: node.status == Some(FileStatus::Deleted),
            path: node.path.clone(),
        });
        if node.kind == NodeKind::Dir {
            flatten(&node.children, depth + 1, out);
        }
    }
}

/// One row: its type's icon slot, its name, and its git state as a word.
pub(super) fn row_view(kit: &Kit, tree_row: &TreeRow, index: usize, selected: bool) -> El {
    let ink = if selected {
        kit.c.text_strong
    } else if tree_row.deleted {
        kit.c.text_faint
    } else if tree_row.status.is_some() {
        kit.c.text_base
    } else {
        kit.c.text_muted
    };
    let mut line = row()
        .key(format!("file-row-{index}"))
        .role("treeitem")
        .level(u32::try_from(tree_row.depth).map_or(1, |depth| depth + 1))
        .label(row_label(tree_row, selected))
        .w_full()
        .gap(6.0)
        .pl(4.0 + INDENT * tree_row.depth as f32)
        .pr(4.0)
        .py(2.0)
        .radius(kit.radius.sm);
    if selected {
        line = line.fill(kit.c.pressed);
    }
    // A directory is always shown expanded, so its caret points down. Every other type's
    // Material Symbols glyph is carried in `TreeRow::icon`; no symbols font is registered yet,
    // so the row draws no second glyph rather than a word that would read as file content.
    if tree_row.icon == DIR_ICON {
        line = line.child(icon(&glyphs::CHEVRON_DOWN, 12.0).color(kit.c.text_faint));
    }
    line = line.child(text(tree_row.name.clone()).font(kit.ty.small).color(ink));
    let word = status_word(tree_row.status);
    if !word.is_empty() {
        line = line.child(spacer()).child(
            text(word)
                .font(kit.ty.small)
                .color(status_colour(kit, tree_row.status)),
        );
    }
    line
}

/// What the row says out loud: its name, its git state and whether it is selected.
fn row_label(tree_row: &TreeRow, selected: bool) -> String {
    let mut label = tree_row.name.clone();
    let word = status_word(tree_row.status);
    if !word.is_empty() {
        label.push_str(", ");
        label.push_str(word);
    }
    if selected {
        label.push_str(", selected");
    }
    label
}

/// Colour reinforces the word; it never carries the meaning on its own.
fn status_colour(kit: &Kit, status: Option<FileStatus>) -> rune::Color {
    match status {
        Some(FileStatus::Added | FileStatus::Untracked) => kit.c.success.fg,
        Some(FileStatus::Modified) => kit.c.warning.fg,
        Some(FileStatus::Deleted | FileStatus::Conflicted) => kit.c.danger.fg,
        None => kit.c.text_faint,
    }
}
