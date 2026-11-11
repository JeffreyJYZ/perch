//! The process side of a tab: what the UI asks the terminal thread and the frames it hears back.

use std::sync::mpsc::Receiver;

use perch_term::{Frame, KeyEvent as TermEvent, Mode, TermHandle, encode_paste};
use rune::{Key, KeyEvent};

use super::TerminalTab;
use crate::shell::term_keys::{control, term_event};

/// What a tab asks of the terminal behind it.
///
/// The UI never touches an emulator: it lives on the terminal thread, and everything a tab wants
/// from it is a question over a channel. This trait is that seam, so a test can drive a tab's
/// paste and scroll rules without a pty — [`TermHandle`] is the real implementation.
pub(crate) trait TermIo {
    /// Send bytes to the shell.
    fn write(&self, bytes: &[u8]);
    /// Encode a press against the live terminal's modes.
    fn key(&self, event: TermEvent) -> Vec<u8>;
    /// Send a press without waiting for its encoding — what typing uses.
    fn send_key(&self, event: TermEvent) {
        self.write(&self.key(event));
    }
    /// Whether the program turned `mode` on.
    fn mode(&self, mode: Mode) -> bool;
    /// Show the grid `offset` rows above the live bottom.
    fn scroll(&self, offset: usize) -> bool;
    /// Tell the shell and the grid about a new size.
    fn resize(&self, cols: u16, rows: u16);
    /// Whether the shell has exited.
    fn exited(&mut self) -> bool;
}

impl TermIo for TermHandle {
    fn write(&self, bytes: &[u8]) {
        TermHandle::write(self, bytes);
    }
    fn key(&self, event: TermEvent) -> Vec<u8> {
        TermHandle::key(self, event)
    }
    fn send_key(&self, event: TermEvent) {
        TermHandle::send_key(self, event);
    }
    fn mode(&self, mode: Mode) -> bool {
        TermHandle::mode(self, mode)
    }
    fn scroll(&self, offset: usize) -> bool {
        TermHandle::scroll(self, offset)
    }
    fn resize(&self, cols: u16, rows: u16) {
        TermHandle::resize(self, cols, rows);
    }
    fn exited(&mut self) -> bool {
        TermHandle::exited(self)
    }
}

/// The process side of a tab: the handle the UI drives and the frames coming back.
pub(crate) struct Session {
    pub(super) io: Box<dyn TermIo>,
    pub(super) frames: Receiver<Frame>,
}

impl Session {
    /// A session over `io`, hearing frames on `frames`.
    pub(crate) fn over(io: Box<dyn TermIo>, frames: Receiver<Frame>) -> Session {
        Session { io, frames }
    }
}

/// How many frames one tab may take in one pump. A burst of output is one frame plus whatever
/// queued behind it, and a tab that flooded must not spend the whole frame draining.
const FRAMES_PER_PUMP: usize = 8;

impl TerminalTab {
    /// Takes the newest frames the tab's thread published, tells the pty about a new grid, and
    /// notices a shell that has exited. Never waits: a tab with nothing new costs one `try_recv`.
    ///
    /// Returns the newest frame, which the shell installs through [`Shell::set_frame`], and
    /// whether the shell has just been found dead.
    ///
    /// [`Shell::set_frame`]: super::super::Shell::set_frame
    pub(crate) fn pump(&mut self, grid: (u16, u16)) -> (Option<Frame>, bool) {
        let Some(session) = self.session.as_mut() else {
            return (None, false);
        };
        let mut newest = None;
        for _ in 0..FRAMES_PER_PUMP {
            match session.frames.try_recv() {
                Ok(frame) => newest = Some(frame),
                Err(_) => break,
            }
        }
        if self.grid != grid {
            session.io.resize(grid.0, grid.1);
            self.grid = grid;
        }
        let ended = self.ended.is_none() && session.io.exited();
        if ended {
            self.ended = Some(super::EXITED.to_string());
        }
        (newest, ended)
    }

    /// Turns one key press into the bytes the shell expects.
    ///
    /// Printable text goes through as the bytes it is; everything a terminal mode can change —
    /// Return, the arrows, Delete, the function keys — is encoded by the terminal's own encoder,
    /// which is the only thing that knows whether the program asked for application cursor keys.
    pub(crate) fn type_key(&mut self, ev: &KeyEvent) -> bool {
        if self.driving().is_none() {
            return false;
        }
        // Typing puts the reader back on the live bottom, as every terminal does: text that
        // arrives into a frozen view is text they cannot see.
        self.bottom();
        let Some(session) = self.driving() else {
            return false;
        };
        match &ev.key {
            Key::Modifier { .. } => false,
            Key::Text(text) => {
                if ev.mods.ctrl {
                    for character in text.chars() {
                        session.io.write(&[control(character)]);
                    }
                } else if ev.mods.alt {
                    // Option is Meta: the same escape prefix a real terminal sends.
                    session.io.write(b"\x1b");
                    session.io.write(text.as_bytes());
                } else {
                    session.io.write(text.as_bytes());
                }
                true
            }
            key => match term_event(key, ev.mods) {
                Some(event) => {
                    session.io.send_key(event);
                    true
                }
                None => false,
            },
        }
    }

    /// Sends `text` to the shell as a paste, marked only when the shell asked for the markers.
    ///
    /// Bracketed paste is the program's own state (DEC 2004), so the tab asks the terminal thread
    /// for it rather than assuming. With it on, [`encode_paste`] wraps the text, which is what
    /// stops a clipboard holding `rm -rf /\n` from running the command the moment it arrives. With
    /// it off the text goes out untouched, because a shell that never enabled the mode would print
    /// the markers as garbage.
    pub(crate) fn paste(&mut self, text: &str) -> bool {
        if self.driving().is_none() {
            return false;
        }
        // What was pasted has to land where the reader can see it, not on the frozen page.
        self.bottom();
        let Some(session) = self.driving() else {
            return false;
        };
        let bracketed = session.io.mode(Mode::BRACKETED_PASTE);
        session.io.write(&encode_paste(text.as_bytes(), bracketed));
        true
    }
}

#[cfg(test)]
mod tests;
