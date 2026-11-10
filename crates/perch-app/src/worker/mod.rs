//! The tool sources on their own thread, and the snapshot the frame reads instead of building.
//!
//! A rebuild loads every plugin and reconciles every configured MCP server, and a server that has
//! to be spawned and handshaked can take seconds. Done on the frame thread that is a frozen
//! window, so the whole [`Sources`] value lives here and the frame only ever reads the snapshot
//! this thread last published: the registry to swap into the live list, the panels to draw and
//! the words to say.
//!
//! Work is a small queue rather than a job per call: [`queue`] holds the slots, and the loop here
//! takes one pass at a time, so a burst of saves during a slow connect is one rebuild against the
//! newest configuration and a frame that pumps while the worker is busy does not queue a thousand
//! of them.

mod queue;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use arc_swap::ArcSwap;
use perch_plugins::PanelEntry;
use perch_store::settings::McpServerConfig;
use perch_store::{SessionLog, SessionMeta, meta_path};

use crate::sources::Sources;

use queue::{Wanted, lock, take};

/// One published state of the tools: everything the frame would otherwise have computed.
pub struct Snapshot {
    /// What every dispatch reads from now on.
    pub registry: perch_tools::ToolRegistry,
    /// One panel per live plugin.
    pub panels: Vec<PanelEntry>,
    /// What did not load, what trapped, and which server is down.
    pub words: Vec<String>,
    /// The MCP servers that answered, in configuration order.
    pub servers: Vec<String>,
}

/// The tool sources, running beside the frame.
pub struct Worker {
    latest: Arc<ArcSwap<Snapshot>>,
    wanted: Arc<Mutex<Wanted>>,
    wake: Sender<()>,
}

impl Worker {
    /// Opens `sources` where they are and starts publishing from them.
    ///
    /// The first snapshot is taken here, on the caller's thread, so the window always opens with
    /// the tools and the words its sources already have. Only later rebuilds are queued.
    pub fn start(mut sources: Sources) -> Worker {
        let latest = Arc::new(ArcSwap::from_pointee(published(&mut sources)));
        let wanted = Arc::new(Mutex::new(Wanted::default()));
        let (wake, woken) = mpsc::channel();
        let running = Arc::clone(&latest);
        let queued = Arc::clone(&wanted);
        std::thread::Builder::new()
            .name("perch-tools".to_string())
            .spawn(move || run(&mut sources, &running, &queued, &woken))
            .expect("spawn the tool source thread");
        Worker {
            latest,
            wanted,
            wake,
        }
    }

    /// The snapshot the worker last published.
    pub fn latest(&self) -> Arc<Snapshot> {
        self.latest.load_full()
    }

    /// Whether anything is wanted or being done. The window keeps drawing while this is true, so
    /// the snapshot that answers a change is read in a frame rather than at the next keypress.
    pub fn busy(&self) -> bool {
        lock(&self.wanted).outstanding
    }

    /// Rebuilds the sources against `servers` and waits for it, answering when the snapshot that
    /// followed it is published. What an explicit reload does: a save may cost its own caller
    /// seconds, and still never costs a frame.
    pub fn rebuild(&self, servers: Vec<McpServerConfig>) {
        let (done, waited) = mpsc::channel();
        self.post(|wanted| {
            wanted.rebuild = Some(servers);
            wanted.waiting.push(done);
        });
        // A worker that stopped is a finished wait: there is nothing left to wait for.
        let _ = waited.recv();
    }

    /// Asks for the same rebuild without waiting for it: what a frame does when the watcher fires.
    pub fn rebuild_soon(&self, servers: Vec<McpServerConfig>) {
        self.post(|wanted| wanted.rebuild = Some(servers));
    }

    /// Asks for the session's new log lines to reach the plugins, without waiting.
    pub fn feed(&self, dir: &Path, meta: &SessionMeta) {
        let log = (dir.to_path_buf(), meta.clone());
        self.post(|wanted| wanted.feed = Some(log));
    }

    /// Queues one change and wakes the worker that there is something to do.
    fn post(&self, change: impl FnOnce(&mut Wanted)) {
        {
            let mut wanted = lock(&self.wanted);
            change(&mut wanted);
            wanted.outstanding = true;
        }
        // The wake goes out after the queue is written, so a worker that was about to park finds
        // the work when it looks and is woken if it has already parked.
        let _ = self.wake.send(());
    }
}

/// The worker thread: take whatever is queued, do it, publish, tell the waiters.
fn run(
    sources: &mut Sources,
    latest: &ArcSwap<Snapshot>,
    wanted: &Mutex<Wanted>,
    wake: &Receiver<()>,
) {
    // One handle per session, so a feed is a read of what is new rather than a replay of the log.
    let mut logs: HashMap<PathBuf, SessionLog> = HashMap::new();
    while wake.recv().is_ok() {
        while let Some(job) = take(wanted) {
            if let Some(servers) = job.rebuild {
                sources.rebuild(&servers);
            }
            if let Some((dir, meta)) = job.feed
                && let Some(log) = log_for(&mut logs, &dir, &meta)
            {
                sources.sync(log);
            }
            latest.store(Arc::new(published(sources)));
            for waiter in job.waiting {
                let _ = waiter.send(());
            }
        }
        // Cleared only once the queue is empty, so a frame that asks while the last job runs keeps
        // the window drawing until the snapshot it is waiting for is published.
        let mut slot = lock(wanted);
        if slot.rebuild.is_none() && slot.feed.is_none() && slot.waiting.is_empty() {
            slot.outstanding = false;
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
fn published(sources: &mut Sources) -> Snapshot {
    let panels = sources.panels();
    sources.settle();
    Snapshot {
        registry: sources.registry().clone(),
        panels,
        words: sources.words(),
        servers: sources.mcp_servers(),
    }
}
