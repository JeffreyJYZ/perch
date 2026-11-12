//! The UI's side of a terminal tab.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Sender},
    },
    time::Duration,
};

use libghostty_vt::terminal::Mode;
use portable_pty::Child;

use super::command::{Cmd, Inbound, KeyEvent};

/// How long a key press waits for the terminal thread to answer.
///
/// The key must be encoded with the terminal's modes, and only the terminal
/// thread may touch it. If that thread is busy with a flood of shell output the
/// wait is abandoned and the stateless encoding is used instead, so a keystroke
/// never hangs the UI on the one thing that can always answer it.
const KEY_TIMEOUT: Duration = Duration::from_millis(250);

/// How long a mode read waits for the terminal thread to answer.
///
/// The question queues behind whatever that thread is doing, and the longest
/// thing it does is build a frame — `BUDGET` in the paint test is 150 ms, so a
/// request that has one frame in front of it still lands inside this wait. Past
/// it the thread is flooded or gone, and a paste that stalls a fifth of a second
/// already reads as a freeze, so the answer is given up instead.
const MODE_TIMEOUT: Duration = Duration::from_millis(250);

/// How long a scroll request waits for the terminal thread to answer.
///
/// Same reasoning as a mode read: the request queues behind at most one frame
/// build, and a caller that blocks longer than a quarter second already reads
/// as a freeze. Past it the answer is `false`, which leaves the view where it
/// is rather than hanging the painter on a thread that is flooded or gone.
const SCROLL_TIMEOUT: Duration = Duration::from_millis(250);

/// The UI's side of a terminal tab.
///
/// Dropping it shuts the tab down: the terminal thread is told to stop and the
/// shell is killed rather than left orphaned.
pub struct TermHandle {
    pub(super) tx: Sender<Inbound>,
    pub(super) dirty: Arc<AtomicBool>,
    pub(super) child: Box<dyn Child + Send + Sync>,
}

impl TermHandle {
    /// Send bytes to the shell.
    pub fn write(&self, bytes: &[u8]) {
        let _ = self.tx.send(Inbound::Cmd(Cmd::Write(bytes.to_vec())));
    }

    /// Tell the shell and the grid about a new size.
    pub fn resize(&self, cols: u16, rows: u16) {
        let _ = self.tx.send(Inbound::Cmd(Cmd::Resize { cols, rows }));
    }

    /// Encode a key press into the bytes the shell expects.
    ///
    /// The encoding is done on the terminal thread, because the escape
    /// sequence for a modified key depends on modes the terminal owns —
    /// application cursor keys, the keypad set, `modifyOtherKeys`, the Kitty
    /// keyboard protocol. This answers the bytes and writes nothing; a
    /// keystroke on its way to the shell uses [`Self::send_key`], which never
    /// waits. The wait is bounded by `KEY_TIMEOUT`; a timeout falls back to the
    /// stateless encoding in [`crate::keys::encode_key`], which is right for
    /// every key the modes do not change.
    #[must_use]
    pub fn key(&self, event: KeyEvent) -> Vec<u8> {
        let (reply, encoded) = mpsc::channel();
        if self
            .tx
            .send(Inbound::Cmd(Cmd::EncodeKey { event, reply }))
            .is_ok()
            && let Ok(bytes) = encoded.recv_timeout(KEY_TIMEOUT)
        {
            return bytes;
        }
        crate::keys::encode_key(event)
    }

    /// A keystroke on its way, without waiting for the encoding.
    ///
    /// The bytes depend on modes only the terminal thread owns, but a key that arrives after a
    /// round trip is a key that feels late: the terminal thread encodes and writes it in one
    /// step, so typing costs one channel send instead of two plus a wait. The same channel
    /// carries text, so a key never overtakes the characters typed before it.
    pub fn send_key(&self, event: KeyEvent) {
        let _ = self.tx.send(Inbound::Cmd(Cmd::Key { event }));
    }

    /// Whether a program turned `mode` on.
    ///
    /// A mode such as bracketed paste ([`Mode::BRACKETED_PASTE`], DEC 2004) is
    /// state the emulator holds and only the terminal thread may read, so the
    /// question goes over the same channel as everything else. The wait is
    /// bounded by `MODE_TIMEOUT`, and `false` is what a timeout answers: it is
    /// also what a shell that never enabled the mode answers, and it is the
    /// safe side, because a paste sent unmarked arrives as text instead of as
    /// markers the shell would print.
    #[must_use]
    pub fn mode(&self, mode: Mode) -> bool {
        let (reply, answer) = mpsc::channel();
        if self
            .tx
            .send(Inbound::Cmd(Cmd::Mode { mode, reply }))
            .is_ok()
            && let Ok(on) = answer.recv_timeout(MODE_TIMEOUT)
        {
            return on;
        }
        false
    }

    /// Show the grid `offset` rows above the live bottom.
    ///
    /// The terminal thread owns the only emulator, so the scroll position lives
    /// there too: this asks that thread to move its viewport and build the frame
    /// for it, which arrives on the frame channel like every other frame. `0`
    /// returns to the live bottom, and the view then follows new output again.
    ///
    /// The wait is bounded by `SCROLL_TIMEOUT`. `false` means the request was
    /// not answered — a flooded or dead terminal thread — and the caller should
    /// leave the view alone: a stale frame is better than a stalled painter.
    #[must_use]
    pub fn scroll(&self, offset: usize) -> bool {
        let (reply, answer) = mpsc::channel();
        if self
            .tx
            .send(Inbound::Cmd(Cmd::Scroll { offset, reply }))
            .is_ok()
            && let Ok(applied) = answer.recv_timeout(SCROLL_TIMEOUT)
        {
            return applied;
        }
        false
    }

    /// Whether a frame has been published since the last call.
    ///
    /// The UI polls this before repainting instead of draining the frame
    /// channel every tick.
    pub fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::AcqRel)
    }

    /// Whether the shell on the other end of the pty has exited.
    ///
    /// A pty outlives the process behind it: the reader sees end of file, the master
    /// still accepts writes and no frame moves again, so nothing else on this handle
    /// changes when the shell dies. The child is the one authority, reaped without
    /// blocking, which is what lets the dock say the shell exited instead of painting
    /// its last frame as if it were still printing.
    pub fn exited(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
    }
}

impl Drop for TermHandle {
    fn drop(&mut self) {
        let _ = self.tx.send(Inbound::Cmd(Cmd::Shutdown));
        let _ = self.child.kill();
    }
}
