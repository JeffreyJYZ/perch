//! The tree as it is drawn: one row per file, each row's press selecting what it names.

use perch_files::DIR_ICON;
use rune::{El, col, text};
use rune_kit::Kit;

use super::FilePanel;
use super::tree::row_view;
use crate::shell::{Live, Shell};

/// The worktree tree: a label, then one row per node, indented by depth.
///
/// A file row's press selects the file it names, which fills the preview beside the tree. The
/// press carries the path the row was built from, so a tree read before a file changed cannot
/// select a path it never showed. A directory row has no press: it is not a file to preview, and
/// pressing it would only make the panel say so.
pub fn files_view<L: Live>(panel: &FilePanel) -> El {
    let kit = Kit::neutral(true);
    let mut tree = col()
        .key("files")
        .role("region")
        .label("Files")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .px(10.0)
        .pt(10.0)
        .pb(10.0)
        .gap(2.0)
        .child(section_label(&kit, "Files", "files-label"));
    if panel.rows.is_empty() {
        tree = tree.child(note(&kit, "No files"));
    }
    for (index, tree_row) in panel.rows.iter().enumerate() {
        let selected = panel.selected_path.as_deref() == Some(tree_row.path.as_path());
        let path = tree_row.path.clone();
        let row = row_view(&kit, tree_row, index, selected);
        if tree_row.icon == DIR_ICON {
            tree = tree.child(row);
            continue;
        }
        tree = tree.child(row.on_press(move |shell: &mut Shell<L>| {
            if let Some(panel) = shell.files_mut() {
                let _ = panel.select(&path);
            }
        }));
    }
    tree
}

/// A section's label: small, uppercase and quieter than the rows under it.
pub(crate) fn section_label(kit: &Kit, label: &str, key: &str) -> El {
    text(label.to_uppercase())
        .key(key)
        .font(kit.ty.small_medium)
        .color(kit.c.text_faint)
}

/// A quiet line of prose, for an empty state or an explanation.
pub(crate) fn note(kit: &Kit, message: &str) -> El {
    text(message.to_owned())
        .font(kit.ty.small)
        .color(kit.c.text_faint)
}
