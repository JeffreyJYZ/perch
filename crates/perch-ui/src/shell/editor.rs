//! The window's press for a file it cannot edit: hand the selected path to the app's editor.

use std::sync::Arc;

use super::live::Live;
use super::state::Shell;
use crate::files::Editor;

impl<L: Live> Shell<L> {
    /// Hands the window the app's editor, once, after the workspace that owns the panel opened.
    ///
    /// A window with no file panel keeps nothing: there is no file to open, so an editor here
    /// would only be a button that cannot work.
    #[must_use]
    pub fn with_editor(mut self, editor: Arc<dyn Editor>) -> Shell<L> {
        if let Some(files) = self.files.as_mut() {
            files.set_editor(editor);
        }
        self
    }

    /// Opens the selected file in the app's editor, as the preview's button does.
    ///
    /// False when nothing was opened: no file selected, no editor attached, or an app that
    /// refused. The reason, when there is one, is the panel's message and the panel draws it.
    pub fn open_in_zed(&mut self) -> bool {
        self.files
            .as_mut()
            .is_some_and(crate::files::FilePanel::open_in_zed)
    }
}
