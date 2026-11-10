//! The terminal thread.
//!
//! `libghostty-vt` handle types are neither `Send` nor `Sync`, so one thread
//! creates, owns and reads the [`Terminal`] for the whole life of a tab. Two
//! channels cross the boundary: [`Inbound`] going in (bytes, commands) and
//! [`Frame`] coming out. `Frame` is plain data and it is the only thing the UI
//! ever touches.

use std::{
    io::{ErrorKind, Read, Write},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::Duration,
};

use libghostty_vt::key::{Key, Mods};
use libghostty_vt::{Terminal, TerminalOptions};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

use crate::frame::{Frame, FrameBuilder, MAX_SCROLLBACK};

/// How long `spawn` waits for the terminal thread to create its emulator.
const READY_TIMEOUT: Duration = Duration::from_secs(5);

/// A key press the terminal should turn into bytes for the shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    /// The physical key.
    pub key: Key,
    /// Modifiers held while it was pressed.
    pub mods: Mods,
}

impl KeyEvent {
    /// A key pressed with no modifiers.
    #[must_use]
    pub fn plain(key: Key) -> Self {
        Self {
            key,
            mods: Mods::empty(),
        }
    }
}

/// Anything that failed while starting a terminal session.
#[derive(Debug, thiserror::Error)]
pub enum TermError {
    /// The PTY could not be created, the shell could not be spawned, or a
    /// handle to it could not be taken.
    #[error("pty: {0}")]
    Pty(String),
    /// The terminal emulator refused a size or could not be created.
    #[error("terminal: {0}")]
    Terminal(#[from] libghostty_vt::Error),
    /// The terminal thread never reported that it had started.
    #[error("the terminal thread did not start")]
    Thread,
}

/// What the terminal thread can be told to do.
enum Cmd {
    /// Write bytes to the shell.
    Write(Vec<u8>),
    /// Resize the PTY and the grid together.
    Resize { cols: u16, rows: u16 },
    /// Stop the thread and let the shell go.
    Shutdown,
}

/// One message into the terminal thread.
enum Inbound {
    /// Bytes the shell wrote.
    Output(Vec<u8>),
    /// A request from the UI.
    Cmd(Cmd),
    /// The PTY reader saw end of file.
    Eof,
}

/// The UI's side of a terminal tab.
///
/// Dropping it shuts the tab down: the terminal thread is told to stop and the
/// shell is killed rather than left orphaned.
pub struct TermHandle {
    tx: Sender<Inbound>,
    dirty: Arc<AtomicBool>,
    child: Box<dyn Child + Send + Sync>,
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
    /// This covers the keys whose encoding is unambiguous without terminal
    /// state. Modifier folding, function keys, bracketed paste and the
    /// application-cursor/application-keypad sequences depend on modes the
    /// terminal owns, so they belong on the terminal thread and land in Task 4
    /// with libghostty's own `key::Encoder`.
    #[must_use]
    pub fn key(&self, event: KeyEvent) -> Vec<u8> {
        let mut out = Vec::new();
        if event.mods.contains(Mods::ALT) {
            out.push(0x1b);
        }
        match event.key {
            Key::Enter => out.push(b'\r'),
            Key::Tab => out.push(b'\t'),
            Key::Backspace => out.push(0x7f),
            Key::Escape => out.push(0x1b),
            Key::Space => out.push(b' '),
            Key::ArrowUp => out.extend_from_slice(b"\x1b[A"),
            Key::ArrowDown => out.extend_from_slice(b"\x1b[B"),
            Key::ArrowRight => out.extend_from_slice(b"\x1b[C"),
            Key::ArrowLeft => out.extend_from_slice(b"\x1b[D"),
            _ => {}
        }
        out
    }

    /// Whether a frame has been published since the last call.
    ///
    /// The UI polls this before repainting instead of draining the frame
    /// channel every tick.
    pub fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::AcqRel)
    }
}

impl Drop for TermHandle {
    fn drop(&mut self) {
        let _ = self.tx.send(Inbound::Cmd(Cmd::Shutdown));
        let _ = self.child.kill();
    }
}

/// Starts a shell in a PTY and an emulator to read it.
pub struct TermSession;

impl TermSession {
    /// Spawn `cwd`'s shell at `cols` x `rows`.
    ///
    /// Returns the handle the UI drives and the channel on which it receives
    /// frames. The first frame is published as soon as the emulator exists, so
    /// a caller can paint an empty grid before the shell has printed anything.
    pub fn spawn(
        cwd: &Path,
        cols: u16,
        rows: u16,
    ) -> Result<(TermHandle, Receiver<Frame>), TermError> {
        let pty = native_pty_system();
        let pair = pty
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| TermError::Pty(error.to_string()))?;

        let mut command = CommandBuilder::new(shell());
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
                )
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

/// `$SHELL` is what the user's terminal already uses; zsh is the fallback
/// because this is a macOS app.
fn shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_owned())
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

#[expect(
    clippy::too_many_arguments,
    reason = "the terminal thread takes ownership of everything the tab owns"
)]
fn run(
    cols: u16,
    rows: u16,
    master: Box<dyn MasterPty + Send>,
    mut writer: Box<dyn Write + Send>,
    inbound: Receiver<Inbound>,
    frames: Sender<Frame>,
    dirty: Arc<AtomicBool>,
    ready: Sender<Result<(), TermError>>,
) {
    // The emulator is created here, not by the caller: `Terminal` is `!Send`
    // and cannot cross the thread boundary.
    let options = TerminalOptions {
        cols,
        rows,
        max_scrollback: MAX_SCROLLBACK,
    };
    let created = Terminal::new(options)
        .map_err(TermError::Terminal)
        .and_then(|terminal| Ok((terminal, FrameBuilder::new()?)));
    let (mut terminal, mut builder) = match created {
        Ok(created) => created,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    if ready.send(Ok(())).is_err() {
        return;
    }
    let _ = publish(&mut builder, &terminal, &frames, &dirty);

    loop {
        let Ok(first) = inbound.recv() else { return };

        // Drain whatever else is already queued so one burst of shell output
        // costs one frame rather than one frame per 8 KiB read.
        let mut batch = vec![first];
        while let Ok(next) = inbound.try_recv() {
            batch.push(next);
        }

        let mut touched = false;
        for message in batch {
            match message {
                Inbound::Output(bytes) => {
                    terminal.vt_write(&bytes);
                    touched = true;
                }
                Inbound::Eof => {}
                Inbound::Cmd(Cmd::Write(bytes)) => {
                    if writer.write_all(&bytes).is_err() {
                        return;
                    }
                    let _ = writer.flush();
                }
                Inbound::Cmd(Cmd::Resize { cols, rows }) => {
                    let size = PtySize {
                        rows,
                        cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    };
                    let _ = master.resize(size);
                    // Cell pixel size is the painter's business; zero means
                    // "unknown", which the emulator accepts.
                    if terminal.resize(cols, rows, 0, 0).is_err() {
                        return;
                    }
                    touched = true;
                }
                Inbound::Cmd(Cmd::Shutdown) => return,
            }
        }

        if touched && publish(&mut builder, &terminal, &frames, &dirty).is_err() {
            return;
        }
    }
}

/// Build a frame and hand it to the UI. `Err` means the UI is gone.
fn publish<'a>(
    builder: &mut FrameBuilder<'a>,
    terminal: &Terminal<'a, '_>,
    frames: &Sender<Frame>,
    dirty: &AtomicBool,
) -> Result<(), ()> {
    let Ok(frame) = builder.build(terminal) else {
        return Ok(());
    };
    frames.send(frame).map_err(drop)?;
    dirty.store(true, Ordering::Release);
    Ok(())
}
