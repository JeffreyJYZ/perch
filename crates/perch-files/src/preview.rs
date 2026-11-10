//! Read-only file preview, capped in bytes and lines.

use std::io::Read;
use std::path::Path;

use crate::status::FileStatus;
use crate::tree::Workspace;
use crate::{Error, Result, language};

/// A preview never reads more than this from disk.
pub const PREVIEW_MAX_BYTES: u64 = 256 * 1024;
/// A preview never returns more than this many lines.
pub const PREVIEW_MAX_LINES: usize = 4_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewText {
    pub lines: Vec<String>,
    pub language: Option<String>,
}

impl Workspace {
    /// Reads one file, refusing anything whose canonical path leaves the root.
    pub fn preview(&self, path: &Path) -> Result<PreviewText> {
        let target = match self.confine(path) {
            Ok(target) => target,
            // A tracked file that is gone from disk is the one case where the confinement
            // failure is not the useful answer: the caller asked about a path the tree is
            // still showing, so say why it cannot be read.
            Err(error) => return Err(self.deletion_error(path).unwrap_or(error)),
        };
        let file = std::fs::File::open(&target)?;
        if !file.metadata()?.is_file() {
            return Err(Error::Message(format!(
                "{} is not a file",
                target.display()
            )));
        }
        let mut bytes = Vec::new();
        file.take(PREVIEW_MAX_BYTES).read_to_end(&mut bytes)?;
        let text = String::from_utf8_lossy(&bytes);
        let lines: Vec<String> = text
            .lines()
            .take(PREVIEW_MAX_LINES)
            .map(str::to_string)
            .collect();
        Ok(PreviewText {
            lines,
            language: language::language_for(&target).map(str::to_string),
        })
    }

    /// Why `path` cannot be read because git reports it deleted, if that is the reason.
    fn deletion_error(&self, path: &Path) -> Option<Error> {
        let relative = self.relative(path)?;
        if self.status_of(&relative) == Some(FileStatus::Deleted) {
            Some(Error::Message(format!(
                "{} is deleted in the worktree",
                relative.display()
            )))
        } else {
            None
        }
    }
}
