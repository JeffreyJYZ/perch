//! perch-update: the update check, and the download staged behind it.
//!
//! This crate checks and stages; it never installs. A perch release is a signed bundle, and
//! replacing one in place rewrites the running binary's own file, its resources and its
//! signature while the process using them is still alive, so the only safe moment to swap is
//! after the app has quit — and quitting is the app's decision, not this crate's. So the
//! boundary is: this crate says "0.1.0-alpha.4 is out, here are the notes, and here is an
//! archive whose SHA-256 matched the manifest, at this path", and the app decides when to stop
//! and what to run next. Folding the download and the swap into one silent step would take
//! that decision away from the one process that knows whether the user is mid-turn, and a swap
//! that fails halfway leaves no working app to report it.
//!
//! **Installing is a manual step.** Nothing here replaces, relaunches or even unpacks the archive:
//! [`check`] answers what is on offer and [`stage`] leaves a verified file where the person can
//! find it. The swap happens after perch has quit, if the person chooses to make it, because the
//! bundle being replaced is the one this process is running from.

mod check;
mod fetch;
mod manifest;
mod version;

use std::path::PathBuf;

pub use check::check;
pub use fetch::{Http, TIMEOUT, Transport, download, stage};
pub use manifest::Manifest;
pub use version::Version;

/// One offer to update: what the window shows before the user agrees to anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    pub version: Version,
    pub url: String,
    pub notes: String,
}

/// Everything this crate can fail at.
///
/// A manifest failure always names the field it came from, because the manifest is written by
/// a release script rather than a person, and "invalid manifest" alone sends that person
/// looking at the whole document.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("manifest field `{field}` is missing")]
    MissingField { field: &'static str },
    #[error("manifest field `{field}` is invalid: {reason}")]
    Invalid { field: &'static str, reason: String },
    #[error("manifest is malformed: {reason}")]
    Shape { reason: String },
    #[error("this release needs macOS {needed}, and this is macOS {running}")]
    UnsupportedOs { needed: u32, running: u32 },
    #[error("the download did not finish within {seconds} s")]
    Timeout { seconds: u64 },
    #[error("the download failed: {reason}")]
    Transport { reason: String },
    #[error(
        "sha256 mismatch for {path}: expected {expected}, got {actual}; the download was deleted"
    )]
    Checksum {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
