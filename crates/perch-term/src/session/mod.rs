//! The terminal thread.
//!
//! `libghostty-vt` handle types are neither `Send` nor `Sync`, so one thread
//! creates, owns and reads the [`Terminal`](libghostty_vt::Terminal) for the
//! whole life of a tab. Two channels cross the boundary:
//! [`Inbound`](command::Inbound) going in (bytes, commands) and
//! [`Frame`](crate::Frame) coming out. `Frame` is plain data and it is the only
//! thing the UI ever touches.
//!
//! The split follows the two sides of that boundary: [`command`] is what may be
//! asked, [`handle`] is the UI's end of it, [`thread`] starts the shell, and
//! [`run`] is the loop that answers.

mod command;
mod handle;
mod run;
mod thread;

pub use command::KeyEvent;
pub use handle::TermHandle;
pub use thread::TermSession;

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
