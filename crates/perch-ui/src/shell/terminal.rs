//! One terminal tab: the last frame its thread published and the shell behind it.
//!
//! A tab also holds the view state the dock reads back: how far the grid is scrolled above the
//! live bottom, and the cells a drag selected. Both belong to one shell's screen, so switching
//! tabs must not carry either across. The code that changes them is split by what it is about:
//! [`io`] is the process side (keys, paste, frames), [`scroll`] the history position in words,
//! and [`pick`] the drag-to-cells mapping and the copy.

use std::sync::mpsc::Receiver;

use perch_term::{CellRange, Frame, TermHandle};

mod io;
mod pick;
mod scroll;

#[cfg(test)]
pub(crate) use io::TermIo;
pub(crate) use pick::cell_at;
pub(crate) use scroll::scroll_words;

/// The grid a tab starts at, before the dock has been measured once.
pub const GRID: (u16, u16) = (100, 12);
/// What the dock says when the shell on the other end of the pty has exited.
pub const EXITED: &str = "the shell exited";
/// What the dock says about a tab whose shell has not printed anything yet.
const WAITING: &str = "waiting";
/// What the dock says about a tab whose shell is still running.
const RUNNING: &str = "running";

/// One terminal tab: its title, the last frame its thread published, and the shell behind it.
///
/// The frame is the whole of what the dock paints, because the terminal's emulator lives on its
/// own thread and only plain snapshots cross to the UI.
pub struct TerminalTab {
    title: String,
    pub(super) frame: Option<Frame>,
    /// The pty this tab drives, and the channel its frames arrive on. Seeded tabs have none.
    session: Option<io::Session>,
    /// Why the shell stopped, in words, once it has. A dead shell says so instead of leaving the
    /// last frame on screen as if it were still printing.
    ended: Option<String>,
    /// The grid the pty was last told about.
    grid: (u16, u16),
    /// Rows the grid is scrolled above the live bottom. `0` follows new output.
    offset: usize,
    /// The cells a drag covered, while there is one.
    selection: Option<CellRange>,
}

impl TerminalTab {
    /// A tab called `title` showing `frame`, or an empty grid while its thread has published
    /// nothing yet.
    pub fn new(title: impl Into<String>, frame: Option<Frame>) -> TerminalTab {
        TerminalTab {
            title: title.into(),
            frame,
            session: None,
            ended: None,
            grid: GRID,
            offset: 0,
            selection: None,
        }
    }

    /// A tab over a spawned shell.
    pub(super) fn live(
        title: impl Into<String>,
        handle: TermHandle,
        frames: Receiver<Frame>,
        grid: (u16, u16),
    ) -> TerminalTab {
        TerminalTab {
            title: title.into(),
            frame: None,
            session: Some(io::Session::over(Box::new(handle), frames)),
            ended: None,
            grid,
            offset: 0,
            selection: None,
        }
    }

    /// A tab whose shell could not be started, saying why instead of showing an empty grid.
    pub(super) fn failed(title: impl Into<String>, words: impl Into<String>) -> TerminalTab {
        TerminalTab {
            title: title.into(),
            frame: None,
            session: None,
            ended: Some(words.into()),
            grid: GRID,
            offset: 0,
            selection: None,
        }
    }

    /// A tab over a terminal thread the caller owns, without a pty.
    ///
    /// This is how a test drives the dock's paste and scroll rules: the thread is a fake that
    /// answers [`TermIo`], so nothing has to spawn a shell to see what a tab writes.
    #[cfg(test)]
    pub(crate) fn over(
        io: Box<dyn TermIo>,
        frames: Receiver<Frame>,
        grid: (u16, u16),
    ) -> TerminalTab {
        TerminalTab {
            title: String::new(),
            frame: None,
            session: Some(io::Session::over(io, frames)),
            ended: None,
            grid,
            offset: 0,
            selection: None,
        }
    }

    /// The tab's title, as the dock shows it.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The last frame the tab's thread published.
    pub fn frame(&self) -> Option<&Frame> {
        self.frame.as_ref()
    }

    /// Why the shell stopped, when it has.
    pub fn ended(&self) -> Option<&str> {
        self.ended.as_deref()
    }

    /// Mark this tab's shell as stopped, saying `words` instead of painting its last frame.
    ///
    /// This is how a test reads what the dock says about a dead shell without spawning one.
    #[cfg(test)]
    pub(crate) fn stop_for_test(&mut self, words: impl Into<String>) {
        self.ended = Some(words.into());
    }

    /// The grid the pty behind this tab was last told about.
    pub fn grid(&self) -> (u16, u16) {
        self.grid
    }

    /// The word the dock shows for this tab.
    pub fn state(&self) -> &'static str {
        if self.ended.is_some() {
            return "exited";
        }
        if self.frame.is_some() {
            RUNNING
        } else {
            WAITING
        }
    }

    /// Whether a shell is still running behind this tab.
    pub(super) fn live_shell(&self) -> bool {
        self.session.is_some() && self.ended.is_none()
    }

    /// Whether this tab's shell has one of its own: the dock's pointer and paste work only on a
    /// tab that can take them.
    pub(super) fn driving(&self) -> Option<&io::Session> {
        self.session.as_ref().filter(|_| self.ended.is_none())
    }
}
