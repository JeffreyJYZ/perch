//! The board: an append-only log the sessions in one project use to talk to each other.
//!
//! One file per project, one line per entry, and one cursor per session, so "what I have not
//! seen" survives a restart and costs one line to ask. Nothing here is a queue: an entry is a
//! fact somebody wrote, and reading it moves only the reader's cursor.
//!
//! [`tools`] is what the harness registers. [`Store`] is the same log without a model in front of
//! it, which is what the tests and the orchestrator's own code use.

mod cancel;
mod cursor;
mod entry;
mod error;
mod session;
mod slug;
mod store;
mod tools;

pub use cancel::{NeverStopped, Stopped};
pub use entry::{Entry, Post};
pub use error::{Error, Result};
pub use session::{SessionName, Slot};
pub use store::{READ_CAP, Read, Store};
pub use tools::{
    WAIT_DEFAULT_MS, WAIT_MAX_MS, tools, tools_dynamic, tools_dynamic_with_cancel,
    tools_with_cancel,
};

/// Unix milliseconds, which is what an entry's `at` holds.
pub(crate) fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}
