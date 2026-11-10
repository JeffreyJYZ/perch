//! The file panel: the worktree tree as rows, with git state said in words.
//!
//! The panel is state; [`files_view`] paints the rows it is given and attaches the one press a
//! row has, so the shell that owns the panel is the only thing that changes it. A row's git state
//! is always a word — "new", "changed", "deleted", "conflict" — and colour only reinforces it.
//! A tracked file git reports deleted keeps its row; selecting it reports the deletion in words
//! instead of previewing an empty file.
//!
//! The Zed handoff is [`Editor`]: one method the app supplies, so the press that opens a file
//! runs the app's `open -a` and a test can stand a fake in its place.

mod editor;
mod tree;
mod view;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use perch_files::{FileStatus, PreviewText, Workspace};

pub use editor::Editor;
pub use tree::TreeRow;
use tree::flatten;
pub use view::files_view;
pub(crate) use view::{note, section_label};

/// The file panel's state: the flattened tree, the one selected path and its preview.
pub struct FilePanel {
    root: PathBuf,
    rows: Vec<TreeRow>,
    selected_path: Option<PathBuf>,
    preview: Option<PreviewText>,
    message: Option<String>,
    /// The app's editor, once it has handed one over. `None` leaves the Zed button disabled
    /// rather than pressing it into nothing.
    editor: Option<Arc<dyn Editor>>,
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
            editor: None,
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
        self.message = None;
        let workspace = Workspace::open(&self.root)?;
        match workspace.preview(path) {
            Ok(preview) => {
                self.selected_path = Some(self.absolute(path));
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

    /// Why the last selection or handoff shows nothing, in words.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// The path the toolbar hands to the editor: only a file that previewed.
    pub fn open_in_zed_request(&self) -> Option<PathBuf> {
        self.selected_path.clone()
    }

    /// The app hands the panel its one editor. Until it does, a handoff is refused in words
    /// rather than silently doing nothing.
    pub fn set_editor(&mut self, editor: Arc<dyn Editor>) {
        self.editor = Some(editor);
    }

    /// The editor this panel holds, so a panel re-read for another worktree keeps it.
    pub fn editor(&self) -> Option<Arc<dyn Editor>> {
        self.editor.clone()
    }

    /// Hands the selected file to the app's editor, and reports whether it went.
    ///
    /// A refusal keeps the preview: the words say why, and the file is still on screen to try
    /// again from. Nothing here runs a command — that is the app's [`Editor`].
    pub fn open_in_zed(&mut self) -> bool {
        let Some(path) = self.open_in_zed_request() else {
            return false;
        };
        let Some(editor) = self.editor.as_ref() else {
            self.message = Some(format!(
                "no editor is attached, so {} stayed here",
                path.display()
            ));
            return false;
        };
        match editor.open_in_zed(&path) {
            Ok(()) => {
                self.message = None;
                true
            }
            Err(words) => {
                self.message = Some(format!("Zed did not open {}: {words}", path.display()));
                false
            }
        }
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
