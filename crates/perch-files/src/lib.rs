//! perch-files: the worktree browser, git status, branch and diff, preview, the Zed handoff, and
//! the tree snapshot `/undo` puts a turn's own files back from.
//!
//! Files are read-only here with one exception: a restore writes back the files of a turn the
//! person asked to undo, and nothing else. The agent and Zed write; this crate only reads
//! otherwise, and refuses anything whose canonical path leaves the workspace root.

mod branch;
mod diff;
mod icons;
mod language;
mod open;
mod preview;
mod process;
pub mod snapshot;
mod status;
mod tree;

use std::path::PathBuf;

pub use branch::{Branch, branch_of};
pub use diff::{DIFF_MAX_BYTES, file_diff, is_repo, worktree_diff};
pub use icons::{Kind, icon_for};
pub use language::language_for;
pub use open::open_in_zed;
pub use preview::{PREVIEW_MAX_BYTES, PREVIEW_MAX_LINES, PreviewText};
pub use status::FileStatus;
pub use tree::{Node, NodeKind, Workspace};

/// Everything this crate can fail at.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{path} is outside the workspace root")]
    OutsideRoot { path: PathBuf },
    #[error("{program} did not finish within {seconds} s")]
    Timeout { program: String, seconds: u64 },
    #[error("{0}")]
    Message(String),
}

pub type Result<T> = std::result::Result<T, Error>;
