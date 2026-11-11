//! The tool sources on their own thread, and the snapshot the frame reads instead of building.
//!
//! A rebuild loads every plugin and reconciles every configured MCP server, and a server that has
//! to be spawned and handshaked can take seconds. Done on the frame thread that is a frozen
//! window, so the whole [`crate::sources::Sources`] value lives here — opened here too, as the
//! first thing this thread does — and the frame only ever reads the snapshot this thread last
//! published: the registry to swap into the live list, the panels to draw and the words to say.
//!
//! Work is a small queue rather than a job per call: [`queue`] holds the slots, and the loop here
//! takes one pass at a time, so a burst of saves during a slow connect is one rebuild against the
//! newest configuration and a frame that pumps while the worker is busy does not queue a thousand
//! of them.

mod body;
mod queue;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

use arc_swap::ArcSwap;
use perch_plugins::PanelEntry;
use perch_store::SessionMeta;
use perch_store::settings::McpServerConfig;

use body::run;
use queue::{PanelAction, Wanted, lock};

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

impl Snapshot {
    /// What a worker publishes before its sources are open: the built-ins, and nothing from disk.
    ///
    /// The built-ins are a registry build with no engine and no server — milliseconds, where a
    /// plugin is a wasm engine and a server is a handshake — so a turn started while the plugins
    /// are still loading can read, write and run commands. What it cannot call yet is a plugin's
    /// own tool, and [`perch_tools::ToolRegistry::call`] answers that with the name it could not
    /// find rather than panicking on a list that is not there.
    fn builtins() -> Snapshot {
        Snapshot {
            registry: perch_agent::Dispatcher::builtin_registry(),
            panels: Vec::new(),
            words: Vec::new(),
            servers: Vec::new(),
        }
    }

    /// A snapshot whose only content besides the built-ins is the reason the sources failed.
    fn failed(error: &crate::Error) -> Snapshot {
        Snapshot {
            words: vec![format!("tools failed to load: {error}")],
            ..Snapshot::builtins()
        }
    }
}

/// The tool sources, running beside the frame.
pub struct Worker {
    latest: Arc<ArcSwap<Snapshot>>,
    wanted: Arc<Mutex<Wanted>>,
    wake: Sender<()>,
}

impl Worker {
    /// Starts the worker on `dir` and `servers`, publishing the built-ins at once.
    ///
    /// Nothing slow happens here. The plugin engine and the MCP handshakes are the seconds that
    /// used to stand between the launch and the first frame — a wasm engine is built for every
    /// plugin, and a server is a process to spawn and a handshake to wait for — so they are done
    /// on this thread instead, and the snapshot they produce replaces the built-ins when it is
    /// ready. The caller draws with the tools every build starts from; [`Worker::busy`] is true
    /// from the start, so frames keep coming until the real one lands.
    pub fn start(dir: PathBuf, servers: Vec<McpServerConfig>) -> Worker {
        let latest = Arc::new(ArcSwap::from_pointee(Snapshot::builtins()));
        let wanted = Arc::new(Mutex::new(Wanted {
            outstanding: true,
            ..Wanted::default()
        }));
        let (wake, woken) = mpsc::channel();
        let running = Arc::clone(&latest);
        let queued = Arc::clone(&wanted);
        std::thread::Builder::new()
            .name("perch-tools".to_string())
            .spawn(move || run(dir, servers, &running, &queued, &woken))
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

    /// Reports a press on a panel control without waiting for the guest's answer.
    ///
    /// The host lives on this thread, so a press is queued like any other change: the worker
    /// answers it, publishes the panels it moved and the words for a refusal, and the frame reads
    /// them on its next pump. A window therefore never waits on a plugin to redraw.
    pub fn panel_action(&self, plugin: &str, control: &str, value: Option<String>) {
        let action = PanelAction {
            plugin: plugin.to_string(),
            control: control.to_string(),
            value,
        };
        self.post(move |wanted| wanted.actions.push(action));
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
