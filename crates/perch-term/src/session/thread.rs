//! Starting a shell in a PTY and the emulator that reads it.

use std::{
    io::{ErrorKind, Read},
    path::Path,
    sync::{
        Arc,
        atomic::AtomicBool,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::Duration,
};

use portable_pty::native_pty_system;

use super::{TermError, TermHandle, command::Inbound, run::run};
use crate::frame::Frame;
use crate::shell::{ShellCommand, missing_program};

/// How long `spawn` waits for the terminal thread to create its emulator.
pub(super) const READY_TIMEOUT: Duration = Duration::from_secs(5);

/// Starts a shell in a PTY and an emulator to read it.
pub struct TermSession;

impl TermSession {
    /// Spawn `cwd`'s shell at `cols` x `rows`.
    ///
    /// Returns the handle the UI drives and the channel on which it receives
    /// frames. The first frame is published as soon as the emulator exists, so
    /// a caller can paint an empty grid before the shell has printed anything.
    ///
    /// This is [`Self::spawn_with_wake`] without a wake callback: a window that
    /// polls the frame channel on a tick needs nothing more.
    pub fn spawn(
        cwd: &Path,
        cols: u16,
        rows: u16,
    ) -> Result<(TermHandle, Receiver<Frame>), TermError> {
        Self::spawn_command(cwd, "", cols, rows)
    }

    /// Spawns the command `line` names, or this machine's own shell when it names none.
    ///
    /// A line that names a program this machine does not have is refused by name before a pty is
    /// opened, so a person reads what is wrong instead of watching a tab die.
    pub fn spawn_command(
        cwd: &Path,
        line: &str,
        cols: u16,
        rows: u16,
    ) -> Result<(TermHandle, Receiver<Frame>), TermError> {
        Self::spawn_command_with_wake(cwd, line, cols, rows, || {})
    }

    /// [`Self::spawn`], calling `wake` after every frame is published.
    ///
    /// The frame channel has no way to say a frame is ready, so a window that
    /// only reads it on a timer shows a shell's output one tick late. `wake` is
    /// that answer: a window passes something that wakes its event loop (Rune's
    /// `AppHandle::wake`), and an echoed character is drawn on the next display
    /// tick instead of the next poll. It runs on the terminal thread, so it must
    /// be cheap and must not block.
    pub fn spawn_with_wake(
        cwd: &Path,
        cols: u16,
        rows: u16,
        wake: impl Fn() + Send + 'static,
    ) -> Result<(TermHandle, Receiver<Frame>), TermError> {
        Self::spawn_command_with_wake(cwd, "", cols, rows, wake)
    }

    /// [`Self::spawn_command`], calling `wake` after every frame is published.
    pub fn spawn_command_with_wake(
        cwd: &Path,
        line: &str,
        cols: u16,
        rows: u16,
        wake: impl Fn() + Send + 'static,
    ) -> Result<(TermHandle, Receiver<Frame>), TermError> {
        if let Some(program) = missing_program(line) {
            return Err(TermError::Missing(program));
        }
        let pty = native_pty_system();
        let pair = pty
            .openpty(pty_size(cols, rows))
            .map_err(|error| TermError::Pty(error.to_string()))?;

        let mut command = ShellCommand::parse(line).builder();
        command.cwd(cwd);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");

        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| TermError::Pty(error.to_string()))?;
        drop(pair.slave);

        let writer = pair
            .master
            .take_writer()
            .map_err(|error| TermError::Pty(error.to_string()))?;
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| TermError::Pty(error.to_string()))?;

        let (tx, inbound) = mpsc::channel();
        let (frames, frame_rx) = mpsc::channel();
        let (ready, ready_rx) = mpsc::channel();
        let dirty = Arc::new(AtomicBool::new(false));

        let reader_tx = tx.clone();
        thread::Builder::new()
            .name("perch-term-reader".into())
            .spawn(move || read_pty(reader, reader_tx))
            .map_err(|error| TermError::Pty(error.to_string()))?;

        let thread_dirty = Arc::clone(&dirty);
        let master = pair.master;
        thread::Builder::new()
            .name("perch-term".into())
            .spawn(move || {
                run(
                    cols,
                    rows,
                    master,
                    writer,
                    inbound,
                    frames,
                    thread_dirty,
                    ready,
                    wake,
                );
            })
            .map_err(|error| TermError::Pty(error.to_string()))?;

        match ready_rx.recv_timeout(READY_TIMEOUT) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(error),
            Err(_) => return Err(TermError::Thread),
        }

        Ok((TermHandle { tx, dirty, child }, frame_rx))
    }
}

/// The size a PTY and the emulator are created at.
pub(super) fn pty_size(cols: u16, rows: u16) -> portable_pty::PtySize {
    portable_pty::PtySize {
        rows,
        cols,
        // Cell pixel size is the painter's business; zero means "unknown",
        // which a PTY and the emulator both accept.
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn read_pty(mut reader: Box<dyn Read + Send>, tx: Sender<Inbound>) {
    let mut buffer = [0u8; 8192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                if tx.send(Inbound::Output(buffer[..read].to_vec())).is_err() {
                    return;
                }
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    let _ = tx.send(Inbound::Eof);
}
