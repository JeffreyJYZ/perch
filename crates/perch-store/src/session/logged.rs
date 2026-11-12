//! Reading a log back: the events it holds, and the lines it could not read.
//!
//! A line is one of three things: the header, an event, or damage — a crash mid-write, a byte
//! somebody edited, a disk that filled. Damage is counted, never repaired and never fatal: a
//! session that lost one line still has the rest of its conversation, and the reader can say how
//! many lines went missing instead of presenting a transcript that quietly starts mid-thought.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use super::{SessionEvent, SessionLog, SessionMeta};
use crate::Result;

/// What a log held when it was read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Logged {
    /// Every event that parsed, in file order.
    pub events: Vec<SessionEvent>,
    /// Lines that are neither the header nor an event, so their content is lost.
    pub skipped: usize,
}

impl SessionLog {
    /// Every readable event in file order, and how many lines could not be read.
    pub fn logged(&self) -> Result<Logged> {
        read(self.path())
    }
}

/// Every readable event of the log at `path`, and how many lines could not be read.
///
/// The first line is the header and is left out when it parses as one. A blank line is nothing
/// rather than damage, so it is neither an event nor a skipped line.
///
/// This reads the file by name instead of through an open [`SessionLog`], and opens it read-only:
/// a fork reads its source here, so copying a session never creates or rewrites the one it copies.
pub(super) fn read(path: &Path) -> Result<Logged> {
    let mut logged = Logged::default();
    let reader = BufReader::new(File::open(path)?);
    for (index, line) in reader.lines().enumerate() {
        let Ok(line) = line else {
            logged.skipped += 1;
            continue;
        };
        if line.trim().is_empty() {
            continue;
        }
        if index == 0 && serde_json::from_str::<SessionMeta>(&line).is_ok() {
            continue;
        }
        match serde_json::from_str::<SessionEvent>(&line) {
            Ok(event) => logged.events.push(event),
            Err(_) => logged.skipped += 1,
        }
    }
    Ok(logged)
}
