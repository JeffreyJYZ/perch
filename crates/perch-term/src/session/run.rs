//! The loop the terminal thread runs, and the two things it does besides
//! answer: resize and publish.

use std::{
    io::Write,
    sync::{
        Arc,
        atomic::AtomicBool,
        atomic::Ordering,
        mpsc::{Receiver, Sender},
    },
};

use libghostty_vt::{Terminal, TerminalOptions};
use portable_pty::MasterPty;

use super::{
    TermError,
    command::{Cmd, Inbound},
    thread::pty_size,
};
use crate::frame::{Frame, FrameBuilder, MAX_SCROLLBACK};

#[expect(
    clippy::too_many_arguments,
    reason = "the terminal thread takes ownership of everything the tab owns"
)]
pub(super) fn run(
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
                Inbound::Cmd(Cmd::Key { event, reply }) => {
                    let bytes = crate::keys::encode_key_for_terminal(event, &terminal);
                    let _ = reply.send(bytes);
                }
                Inbound::Cmd(Cmd::Mode { mode, reply }) => {
                    // A read changes nothing on screen, so it leaves `touched`
                    // alone: answering must never cost a frame.
                    let _ = reply.send(terminal.mode(mode).unwrap_or(false));
                }
                Inbound::Cmd(Cmd::Resize { cols, rows }) => {
                    // The PTY first, so the shell's SIGWINCH sees the size its
                    // next prompt will wrap at, then the grid. A frame is built
                    // only after both, which is why this loop marks the batch
                    // touched instead of publishing here: a frame published in
                    // between would carry the old width with a new prompt.
                    if resize(master.as_ref(), &mut terminal, cols, rows) {
                        touched = true;
                    }
                }
                Inbound::Cmd(Cmd::Shutdown) => return,
            }
        }

        if touched && publish(&mut builder, &terminal, &frames, &dirty).is_err() {
            return;
        }
    }
}

/// Tell the PTY and then the grid about a new size.
///
/// The order is the contract: a shell that is told second may wrap its prompt
/// for the new width while the grid still holds the old one, and the frame the
/// UI then reads shows text the user never sees in a real terminal. Returns
/// whether the grid accepted the size, because a frame is only worth building
/// when it did; a refused PTY resize is reported by the shell's next prompt and
/// must not stop the grid from following.
fn resize(master: &dyn MasterPty, terminal: &mut Terminal<'_, '_>, cols: u16, rows: u16) -> bool {
    let _ = master.resize(pty_size(cols, rows));
    terminal.resize(cols, rows, 0, 0).is_ok()
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
