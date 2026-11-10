//! The session log as an event feed.
//!
//! A plugin sees the session's events without the app and the plugin host knowing about each
//! other: the app appends every event to the session's JSONL log and flushes each line, so a reader
//! that remembers its byte offset sees exactly the events it has not dispatched yet. That makes the
//! log the tap, which is why nothing here re-reads what it already delivered and why a line that is
//! still being written is left for the next read instead of being half-parsed.
//!
//! One offset per log path, so switching sessions costs nothing and replays nothing: each session's
//! events are delivered once, in the order the app wrote them.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use perch_store::{SessionEvent, SessionLog};

/// How much of each session log has already been dispatched.
#[derive(Debug, Default)]
pub struct Feed {
    /// The byte offset read so far, per log path.
    seen: HashMap<PathBuf, u64>,
}

impl Feed {
    /// The events appended since the last call, oldest first.
    ///
    /// A log that has shrunk is a different log under the same name — a session deleted and
    /// recreated — so it is read from the start rather than from an offset past its end.
    pub fn since(&mut self, log: &SessionLog) -> Vec<SessionEvent> {
        let path = log.path().to_path_buf();
        let Ok(mut file) = std::fs::File::open(&path) else {
            return Vec::new();
        };
        let len = file.metadata().map(|meta| meta.len()).unwrap_or(0);
        let mut from = self.seen.get(&path).copied().unwrap_or(0);
        if len < from {
            from = 0;
        }
        if file.seek(SeekFrom::Start(from)).is_err() {
            return Vec::new();
        }
        let mut body = String::new();
        if file.read_to_string(&mut body).is_err() {
            return Vec::new();
        }

        // A last line with no newline after it is one the writer has not finished, so it is read
        // next time. The header line decodes as no event at all and is skipped by the same rule
        // that drops a torn line.
        let complete = body.rfind('\n').map_or(0, |at| at + 1);
        let mut events = Vec::new();
        for line in body[..complete].lines() {
            if let Ok(event) = serde_json::from_str::<SessionEvent>(line) {
                events.push(event);
            }
        }
        self.seen.insert(path, from + complete as u64);
        events
    }
}
