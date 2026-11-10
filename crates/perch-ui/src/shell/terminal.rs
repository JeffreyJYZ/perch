//! One terminal tab: the last frame its thread published and the shell behind it.

use std::sync::mpsc::Receiver;

use perch_term::{Frame, TermHandle};
use rune::{Key, KeyEvent};

use super::term_keys::{control, term_event};

/// The grid a tab starts at, before the dock has been measured once.
pub const GRID: (u16, u16) = (100, 12);
/// How many frames one tab may take in one pump. A burst of output is one frame plus whatever
/// queued behind it, and a tab that flooded must not spend the whole frame draining.
const FRAMES_PER_PUMP: usize = 8;
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
    session: Option<TermSlot>,
    /// Why the shell stopped, in words, once it has. A dead shell says so instead of leaving the
    /// last frame on screen as if it were still printing.
    ended: Option<String>,
    /// The grid the pty was last told about.
    grid: (u16, u16),
}

/// The process side of a tab: the handle the UI drives and the frames coming back.
struct TermSlot {
    handle: TermHandle,
    frames: Receiver<Frame>,
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
            session: Some(TermSlot { handle, frames }),
            ended: None,
            grid,
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

    /// Takes the newest frames the tab's thread published, tells the pty about a new grid, and
    /// notices a shell that has exited. Never waits: a tab with nothing new costs one `try_recv`.
    ///
    /// Returns the newest frame, which the shell installs through [`Shell::set_frame`], and
    /// whether the shell has just been found dead.
    ///
    /// [`Shell::set_frame`]: super::Shell::set_frame
    pub(super) fn pump(&mut self, grid: (u16, u16)) -> (Option<Frame>, bool) {
        let Some(slot) = self.session.as_mut() else {
            return (None, false);
        };
        let mut newest = None;
        for _ in 0..FRAMES_PER_PUMP {
            match slot.frames.try_recv() {
                Ok(frame) => newest = Some(frame),
                Err(_) => break,
            }
        }
        if self.grid != grid {
            slot.handle.resize(grid.0, grid.1);
            self.grid = grid;
        }
        let ended = self.ended.is_none() && slot.handle.exited();
        if ended {
            self.ended = Some(EXITED.to_string());
        }
        (newest, ended)
    }

    /// Turns one key press into the bytes the shell expects.
    ///
    /// Printable text goes through as the bytes it is; everything a terminal mode can change —
    /// Return, the arrows, Delete, the function keys — is encoded by the terminal's own encoder,
    /// which is the only thing that knows whether the program asked for application cursor keys.
    pub(super) fn type_key(&mut self, ev: &KeyEvent) -> bool {
        if self.ended.is_some() {
            return false;
        }
        let Some(slot) = self.session.as_mut() else {
            return false;
        };
        match &ev.key {
            Key::Modifier { .. } => false,
            Key::Text(text) => {
                if ev.mods.ctrl {
                    for character in text.chars() {
                        slot.handle.write(&[control(character)]);
                    }
                } else if ev.mods.alt {
                    // Option is Meta: the same escape prefix a real terminal sends.
                    slot.handle.write(b"\x1b");
                    slot.handle.write(text.as_bytes());
                } else {
                    slot.handle.write(text.as_bytes());
                }
                true
            }
            key => match term_event(key, ev.mods) {
                Some(event) => {
                    slot.handle.write(&slot.handle.key(event));
                    true
                }
                None => false,
            },
        }
    }
}
