//! What the worker has been asked for, and how one pass of it is taken.
//!
//! Work is a set of slots rather than a job per call: a rebuild is one slot the newest request
//! overwrites, a feed is one slot, and the waiters are whoever asked for a rebuild and is blocked
//! until it lands. A burst of saves during a slow connect is therefore one rebuild against the
//! newest configuration, not a queue of them.

use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Mutex, MutexGuard};

use perch_store::SessionMeta;
use perch_store::settings::McpServerConfig;

/// What the worker has been asked for and has not done yet.
#[derive(Default)]
pub(super) struct Wanted {
    /// The servers a rebuild must reconcile. Last write wins: two saves during one connect are one
    /// rebuild against the newer configuration.
    pub(super) rebuild: Option<Vec<McpServerConfig>>,
    /// The session whose new log lines the plugins have not been given.
    pub(super) feed: Option<(PathBuf, SessionMeta)>,
    /// The presses on panel controls the plugins have not been told about, in order. Every press
    /// is its own event — pressing a button twice must count twice — so these are queued rather
    /// than held in one slot.
    pub(super) actions: Vec<PanelAction>,
    /// Who asked for a rebuild and is waiting for it to be done.
    pub(super) waiting: Vec<Sender<()>>,
    /// Whether anything is wanted or being done: what the window reads to keep a frame coming
    /// until the snapshot that answers it has been published.
    pub(super) outstanding: bool,
}

/// One press on a control a plugin's panel declared, waiting for the worker.
pub(super) struct PanelAction {
    pub(super) plugin: String,
    pub(super) control: String,
    /// The text an input was submitted with; a button press has none.
    pub(super) value: Option<String>,
}

/// One pass of work taken out of the queue.
pub(super) struct Job {
    pub(super) rebuild: Option<Vec<McpServerConfig>>,
    pub(super) feed: Option<(PathBuf, SessionMeta)>,
    pub(super) actions: Vec<PanelAction>,
    pub(super) waiting: Vec<Sender<()>>,
}

/// Takes one pass of work out of the queue, or `None` when there is none.
pub(super) fn take(wanted: &Mutex<Wanted>) -> Option<Job> {
    let mut slot = lock(wanted);
    if slot.rebuild.is_none()
        && slot.feed.is_none()
        && slot.actions.is_empty()
        && slot.waiting.is_empty()
    {
        return None;
    }
    Some(Job {
        rebuild: slot.rebuild.take(),
        feed: slot.feed.take(),
        actions: std::mem::take(&mut slot.actions),
        waiting: std::mem::take(&mut slot.waiting),
    })
}

/// A lock that survives a thread that panicked while holding it: the queue is plain data, so the
/// next pass is still valid work rather than a poisoned state.
pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
