//! Read-only file preview, capped in bytes and lines.

use std::io::Read;
use std::path::Path;

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
        let target = self.confine(path)?;
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
}
