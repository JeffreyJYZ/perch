//! The file panel: the worktree tree as rows, with git state said in words.
//!
//! The panel is state and [`files_view`] is pure: it paints the rows it is given and takes no
//! state handle, so the shell that owns the panel attaches the presses. A row's git state is
//! always a word — "new", "changed", "deleted", "conflict" — and colour only reinforces it.
//! A tracked file git reports deleted keeps its row; selecting it reports the deletion in
//! words instead of previewing an empty file.

use std::path::{Path, PathBuf};

use perch_files::{DIR_ICON, FileStatus, Node, NodeKind, PreviewText, Workspace, icon_for};
use rune::{El, col, icon, row, spacer, text};
use rune_kit::Kit;
use rune_kit::glyphs;

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

/// The file panel's state: the flattened tree, the one selected path and its preview.
pub struct FilePanel {
    root: PathBuf,
    rows: Vec<TreeRow>,
    selected_path: Option<PathBuf>,
    preview: Option<PreviewText>,
    message: Option<String>,
}

impl FilePanel {
    /// Reads `workspace` once: the walk, git state and the rows the tree shows.
    pub fn new(workspace: &Workspace) -> FilePanel {
        let mut rows = Vec::new();
        flatten(&workspace.tree(), 0, &mut rows);
        FilePanel {
            root: workspace.root().to_path_buf(),
            rows,
            selected_path: None,
            preview: None,
            message: None,
        }
    }

    /// Every row, flattened in tree order with each row's depth.
    pub fn rows(&self) -> Vec<TreeRow> {
        self.rows.clone()
    }

    /// Selects `path` and previews it, or records why it cannot be previewed.
    ///
    /// A deleted file is the ordinary failure here: `preview` names the deletion, the panel
    /// keeps that sentence for display and the selection stays empty, so the toolbar never
    /// offers a path that is not there.
    pub fn select(&mut self, path: &Path) -> perch_files::Result<()> {
        self.preview = None;
        self.selected_path = None;
        let workspace = Workspace::open(&self.root)?;
        match workspace.preview(path) {
            Ok(preview) => {
                self.selected_path = Some(self.absolute(path));
                self.message = None;
                self.preview = Some(preview);
                Ok(())
            }
            Err(error) => {
                self.message = Some(error.to_string());
                Err(error)
            }
        }
    }

    /// The selected file's preview, when one was read.
    pub fn selected(&self) -> Option<&PreviewText> {
        self.preview.as_ref()
    }

    /// The selected file's path, absolute, when one was read.
    pub fn selected_path(&self) -> Option<&Path> {
        self.selected_path.as_deref()
    }

    /// Why the last selection shows no preview, in words.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// The path the toolbar hands to `perch_files::open_in_zed`: only a file that previewed.
    pub fn open_in_zed_request(&self) -> Option<PathBuf> {
        self.selected_path.clone()
    }

    /// `path` as this panel's root resolves it, without touching the filesystem: a tree node's
    /// path must stay usable after the file is gone.
    fn absolute(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        }
    }
}

/// One file's git state as the word the panel shows, empty when git has nothing to say.
pub fn status_word(status: Option<FileStatus>) -> &'static str {
    match status {
        Some(FileStatus::Added | FileStatus::Untracked) => "new",
        Some(FileStatus::Modified) => "changed",
        Some(FileStatus::Deleted) => "deleted",
        Some(FileStatus::Conflicted) => "conflict",
        None => "",
    }
}

/// The worktree tree: a label, then one row per node, indented by depth.
pub fn files_view(panel: &FilePanel) -> El {
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
        tree = tree.child(row_view(&kit, tree_row, index, selected));
    }
    tree
}

/// One row: its type's icon slot, its name, and its git state as a word.
fn row_view(kit: &Kit, tree_row: &TreeRow, index: usize, selected: bool) -> El {
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

fn flatten(nodes: &[Node], depth: usize, out: &mut Vec<TreeRow>) {
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
