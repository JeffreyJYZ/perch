//! Opening a folder into a session: the picker the window asks, and the words when it answers
//! nothing.
//!
//! A window that opened with no folder is a normal window, so this is one press: Rune's native
//! folder dialog when the window runs for real, a [`FileDialogs`] list a test injected when it
//! does not. Both answers land in [`Shell::adopt`], so the session, its row and its file tree are
//! made one way whichever picker asked.

use std::future::Future;
use std::path::PathBuf;
use std::task::{Context, Poll, Waker};

use rune::{FileDialogError, FileDialogOptions, FileDialogs, Tx};

use super::live::Live;
use super::state::Shell;

/// What the window knows about folders it has not opened.
#[derive(Default)]
pub(super) struct Folder {
    /// Why the window has no folder, or why the last pick opened none, in words.
    pub(super) notice: Option<String>,
    /// The picker a test injected. `None` asks Rune's host for its native dialog.
    pub(super) dialogs: Option<FileDialogs>,
}

impl<L: Live> Shell<L> {
    /// Asks for a folder and makes it this window's session.
    ///
    /// The native dialog is asked for through `tx`, so the window keeps drawing and the answer is
    /// an action on a later frame. A scripted dialog answers at once and is read here.
    pub fn open_folder(&mut self, tx: &mut Tx) {
        let options = FileDialogOptions {
            title: "Open folder".to_string(),
            ..Default::default()
        };
        let Some(dialogs) = self.folder.dialogs.clone() else {
            tx.open_folder(options, |shell: &mut Shell<L>, answer, _| {
                shell.adopt(answer);
            });
            return;
        };
        self.adopt(answered(dialogs.open_folder(options)));
    }

    /// Why the window has no folder, or why the last pick opened none, in words.
    pub fn notice(&self) -> Option<&str> {
        self.folder.notice.as_deref()
    }

    /// The words the window opens with when the launch directory was not a project.
    #[must_use]
    pub fn with_notice(mut self, words: impl Into<String>) -> Shell<L> {
        self.folder.notice = Some(words.into());
        self
    }

    /// A scripted picker, so a test presses Open folder and no real panel ever asks for anything.
    #[must_use]
    pub fn with_folder_dialogs(mut self, dialogs: FileDialogs) -> Shell<L> {
        self.folder.dialogs = Some(dialogs);
        self
    }

    /// Makes the picked folder a session: its row in the list, its tree in the panel, and the
    /// app's words when nothing opened. A cancelled dialog changes nothing at all.
    fn adopt(&mut self, answer: Result<Option<PathBuf>, FileDialogError>) {
        self.folder.notice = None;
        let path = match answer {
            Ok(Some(path)) => path,
            Ok(None) => return,
            Err(error) => {
                self.folder.notice = Some(error.to_string());
                return;
            }
        };
        let Some(live) = self.live.as_mut() else {
            self.folder.notice = Some("no app is running, so no folder was opened".to_string());
            return;
        };
        match live.open_folder(&path) {
            Ok((index, meta)) => {
                self.sessions.push(meta);
                // The app made this session active; an index past the end would name a row the
                // list does not hold, so it is clamped to the row just added.
                self.active = index.min(self.sessions.len() - 1);
                self.reopen_files();
                self.refresh();
            }
            Err(words) => self.folder.notice = Some(words),
        }
    }
}

/// A dialog future read on its first poll: a scripted one is ready at once, and a native one is
/// never injected here. A future that waits is a pick that did not happen, not a frame that hangs.
fn answered(
    future: impl Future<Output = Result<Option<PathBuf>, FileDialogError>>,
) -> Result<Option<PathBuf>, FileDialogError> {
    let mut future = std::pin::pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut cx) {
        Poll::Ready(answer) => answer,
        Poll::Pending => Err(FileDialogError(
            "the folder picker did not answer".to_string(),
        )),
    }
}
