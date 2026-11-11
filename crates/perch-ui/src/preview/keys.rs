//! Escape leaves the preview, as the header's button does.

use rune::{Key, KeyEvent};

use crate::shell::{Live, Shell, Waiting};

/// Whether `ev` closes the preview. Only a bare Escape does, and only when nothing else owns it:
/// an approval or a question answers Escape itself, and an open answer field takes it back first.
pub fn preview_key<L: Live>(shell: &mut Shell<L>, ev: &KeyEvent) -> bool {
    if ev.mods.cmd || ev.mods.ctrl || ev.mods.alt || ev.mods.logo {
        return false;
    }
    if !matches!(ev.key, Key::Escape) || shell.typing() || shell.waiting() != Waiting::Nothing {
        return false;
    }
    shell.close_preview()
}

impl<L: Live> Shell<L> {
    /// Leaves the preview for the transcript, and reports whether one was showing. The tree keeps
    /// its selection, so the same file is one press from coming back.
    pub fn close_preview(&mut self) -> bool {
        match self.files_mut() {
            Some(panel) if panel.is_open() => {
                panel.close();
                true
            }
            _ => false,
        }
    }
}
