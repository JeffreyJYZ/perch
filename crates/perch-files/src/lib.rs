//! perch-files: the worktree browser, git status, branch and diff, preview, and the Zed handoff.
//!
//! Files are read-only here. The agent and Zed write; this crate only reads, and refuses
//! anything whose canonical path leaves the workspace root.

mod branch;
mod diff;
mod icons;
mod language;
mod open;
mod preview;
mod process;
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
