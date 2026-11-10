//! Opening a file in the editor, behind one method so a test never runs a command.
//!
//! Files are read-only in perch: the one thing the window asks of the app for a file is that
//! something else opens it. The app supplies that, and the fake a test supplies records it.

use std::path::Path;

/// The app's one way into the editor.
pub trait Editor {
    /// Opens `path` in the editor. The error is why it did not open, in words.
    fn open_in_zed(&self, path: &Path) -> Result<(), String>;
}
