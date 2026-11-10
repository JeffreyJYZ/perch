//! The Zed handoff: one path as one argument, never a shell string.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::process;
use crate::{Error, Result};

pub(crate) const OPEN_TIMEOUT: Duration = Duration::from_secs(30);

/// Opens `path` in Zed.
pub fn open_in_zed(path: &Path) -> Result<()> {
    let target = std::fs::canonicalize(path)
        .map_err(|e| Error::Message(format!("cannot open {}: {e}", path.display())))?;
    if !target.is_file() {
        return Err(Error::Message(format!(
            "{} is not a file",
            target.display()
        )));
    }
    let mut cmd = Command::new("open");
    // The canonical path is absolute, so no argument can be mistaken for a flag.
    cmd.arg("-a").arg("Zed").arg(&target);
    let captured = process::run(cmd, OPEN_TIMEOUT)?;
    if captured.ok {
        Ok(())
    } else {
        Err(Error::Message(format!(
            "open -a Zed {} failed: {}",
            target.display(),
            captured.stderr.trim()
        )))
    }
}
