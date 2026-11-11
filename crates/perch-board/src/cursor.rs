//! The per-session cursor: one number beside the log, so a restart does not re-read the board.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use crate::error::Result;

/// The last id this session has read. Nothing there yet means it has read nothing.
pub fn read(path: &Path) -> Result<u64> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text.trim().parse().unwrap_or(0)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(0),
        Err(error) => Err(error.into()),
    }
}

/// Moves the cursor forward, never back: a second read cannot unread the board for anyone.
pub fn write(path: &Path, id: u64) -> Result<()> {
    if id <= read(path)? {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, format!("{id}\n"))?;
    Ok(())
}
