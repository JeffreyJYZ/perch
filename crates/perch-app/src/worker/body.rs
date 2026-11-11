//! The worker thread's body: open the sources, publish them, then take one pass of queued work
//! at a time, publishing after each.
//!
//! Split from [`Worker`](super::Worker) so the queue's contract and the thread that drains it are
//! each readable on their own: this file never decides what is wanted, only what to do with a
//! pass that was taken.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use arc_swap::ArcSwap;
use perch_store::settings::McpServerConfig;
use perch_store::{SessionLog, SessionMeta, meta_path};

use super::Snapshot;
use super::queue::{settle, take};
use crate::sources::Sources;

/// The worker thread: open the sources, publish them, then take whatever is queued.
///
/// The open is here rather than on the caller's thread because it is the slowest step in opening
/// the window: a wasm engine is built for the plugins and an MCP server is a process to spawn and
/// a handshake to wait for. The caller published an empty snapshot before this thread started, so
/// the window is already drawing, and `busy()` keeps frames coming until the real one lands.
pub(super) fn run(
    dir: PathBuf,
    servers: Vec<McpServerConfig>,
    latest: &ArcSwap<Snapshot>,
    wanted: &Mutex<super::queue::Wanted>,
    wake: &Receiver<()>,
) {
    // One handle per session, so a feed is a read of what is new rather than a replay of the log.
    let mut logs: HashMap<PathBuf, SessionLog> = HashMap::new();
    let mut sources = open(&dir, &servers, latest);
    if let Some(sources) = sources.as_mut() {
        latest.store(Arc::new(published(sources)));
    }
    // The empty snapshot was published by the caller, so the flag that keeps frames coming is
    // cleared here: either this thread is now idle, or a post has already set it again.
    settle(wanted);
    while wake.recv().is_ok() {
        while let Some(job) = take(wanted) {
            // Sources that failed to open are tried again on the next change: a save is the
            // reason a person expects a plugin to come back, and the words still say it failed.
            if sources.is_none() {
                sources = open(&dir, &servers, latest);
            }
            if let Some(sources) = sources.as_mut() {
                // The presses go first: each one was accepted against the panel the window drew,
                // so it is answered by the host that drew it even when a rebuild is also queued.
                for action in job.actions {
                    // A refusal is not dropped: the source keeps it as column words, which is
                    // where a press that reached no plugin is read.
                    let _ = sources.panel_action(
                        &action.plugin,
                        &action.control,
                        action.value.as_deref(),
                    );
                }
                if let Some(servers) = job.rebuild {
                    sources.rebuild(&servers);
                }
                if let Some((dir, meta)) = job.feed
                    && let Some(log) = log_for(&mut logs, &dir, &meta)
                {
                    sources.sync(log);
                }
                latest.store(Arc::new(published(sources)));
            }
            for waiter in job.waiting {
                let _ = waiter.send(());
            }
        }
        // Cleared only once the queue is empty, so a frame that asks while the last job runs keeps
        // the window drawing until the snapshot it is waiting for is published.
        settle(wanted);
    }
}

/// The sources over `dir`, or the reason they could not be opened.
///
/// A plugin engine that will not build is not a window that will not open: nothing is produced
/// but the words, which reach the column as the note saying what is missing.
fn open(dir: &Path, servers: &[McpServerConfig], latest: &ArcSwap<Snapshot>) -> Option<Sources> {
    match Sources::open(dir.to_path_buf(), servers) {
        Ok(sources) => Some(sources),
        Err(error) => {
            latest.store(Arc::new(Snapshot::failed(&error)));
            None
        }
    }
}

/// The session's log as this thread sees it, opened once and kept.
///
/// A second handle on a file the session appends to is safe: this one only reads through
/// [`Sources::sync`], which opens the file itself and remembers how far it read.
fn log_for<'log>(
    logs: &'log mut HashMap<PathBuf, SessionLog>,
    dir: &Path,
    meta: &SessionMeta,
) -> Option<&'log mut SessionLog> {
    let path = meta_path(dir, &meta.id);
    if !logs.contains_key(&path) {
        logs.insert(path.clone(), SessionLog::open(dir, meta.clone()).ok()?);
    }
    logs.get_mut(&path)
}

/// Everything the frame draws, computed from the sources as they are now.
///
/// The panels are asked first because a plugin that traps while its panel is drawn is recorded
/// there; the release then lands in this same pass, so the registry is never published offering a
/// spec whose tool has just gone.
pub(super) fn published(sources: &mut Sources) -> Snapshot {
    let panels = sources.panels();
    sources.settle();
    Snapshot {
        registry: sources.registry().clone(),
        panels,
        words: sources.words(),
        servers: sources.mcp_servers(),
    }
}
