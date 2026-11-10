//! The plugins a running app has: the host, the registry their tools landed in, and the words for
//! whatever did not load or had to be taken away.
//!
//! This is the seam the window uses, and it never fails and never panics. A directory that does not
//! exist is no plugins rather than an error, because most windows have none; a component that will
//! not load is one line of words beside the ones that still load, because a plugin a person
//! installed must not stop the window from opening.
//!
//! The registry is the caller's, built by the caller: a plugin tool registers into exactly the
//! registry the built-ins live in, so `specs()` offers both and `call` dispatches both by name
//! through one path.

use std::path::Path;
use std::sync::Mutex;

use perch_store::{SessionEvent, SessionLog};
use perch_tools::ToolRegistry;

use crate::bounds::lock;
use crate::feed::Feed;
use crate::host::PluginHost;
use crate::panel::PanelModel;

/// One plugin's panel, and the plugin that declares it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanelEntry {
    /// The plugin's name: its wasm file's stem.
    pub plugin: String,
    /// What that plugin says its panel shows right now.
    pub model: PanelModel,
}

/// Every loaded plugin, the registry they registered into, and what went wrong.
pub struct Loaded {
    host: PluginHost,
    registry: ToolRegistry,
    /// One line per file that did not load, in load order.
    failures: Vec<String>,
    /// The plugins a trap has taken since this window started, in the order they went. Behind a
    /// lock because asking for a panel is `&self` and can be what takes a plugin away.
    disabled: Mutex<Vec<String>>,
    /// Which of each session log's events have been dispatched.
    feed: Feed,
}

impl Loaded {
    /// Loads every `*.wasm` in `dir` into `registry`.
    pub fn open(dir: &Path, registry: ToolRegistry) -> Loaded {
        if !dir.is_dir() {
            return Self::over(PluginHost::empty(), registry, Vec::new());
        }
        let mut registry = registry;
        let (host, failures) = PluginHost::load_all(dir, &mut registry);
        Self::over(host, registry, failures)
    }

    fn over(host: PluginHost, registry: ToolRegistry, failures: Vec<String>) -> Loaded {
        Loaded {
            host,
            registry,
            failures,
            disabled: Mutex::new(Vec::new()),
            feed: Feed::default(),
        }
    }

    /// The registry the tools landed in: the built-ins, then one tool per declared plugin tool.
    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    /// How many plugins are still loaded. A disabled one does not count.
    pub fn len(&self) -> usize {
        self.host.len()
    }

    pub fn is_empty(&self) -> bool {
        self.host.is_empty()
    }

    /// The panels to draw, one per live plugin, in load order.
    ///
    /// The model is asked for every time, so the column is never one event behind. Asking can
    /// disable a plugin that traps while producing it, so the note is taken here too — otherwise a
    /// panel that trapped would leave no trace until the next session event arrived.
    pub fn panels(&self) -> Vec<PanelEntry> {
        let mut panels = Vec::new();
        for plugin in self.host.plugin_names() {
            if let Some(model) = self.host.panel_model(&plugin) {
                panels.push(PanelEntry { plugin, model });
            }
        }
        self.note_disabled();
        panels
    }

    /// One session event to every live plugin, answering whether any of them trapped on it.
    ///
    /// Every plugin is offered every event, which is the host's rule: a plugin that does not care
    /// decodes nothing.
    pub fn dispatch(&mut self, event: &SessionEvent) -> bool {
        self.host.dispatch_event(event);
        self.settle()
    }

    /// Delivers the events this log has grown by since the last call, and takes away the tools of
    /// any plugin that trapped on one of them.
    ///
    /// The log is the app's own durable record, written and flushed event by event, so this is the
    /// same sequence the app folded into its transcript.
    pub fn sync(&mut self, log: &SessionLog) -> bool {
        let events = self.feed.since(log);
        for event in &events {
            self.host.dispatch_event(event);
        }
        self.settle() || !events.is_empty()
    }

    /// Unregisters the tools of every plugin a trap disabled, answering how many went, and records
    /// the plugin's name so the column can say which one it was.
    pub fn release(&mut self) -> usize {
        self.note_disabled();
        self.host.release(&mut self.registry)
    }

    /// One line per file that did not load, in load order.
    pub fn failures(&self) -> &[String] {
        &self.failures
    }

    /// Every word the window should say about plugins: what did not load, and what had to go.
    pub fn words(&self) -> Vec<String> {
        let mut words = self.failures.clone();
        for plugin in lock(&self.disabled).iter() {
            words.push(format!(
                "plugin `{plugin}` was disabled: its guest trapped, so its tools are gone"
            ));
        }
        words
    }

    /// Takes the tools of anything that trapped and reports whether that changed the world.
    fn settle(&mut self) -> bool {
        let noted = self.note_disabled();
        noted | (self.host.release(&mut self.registry) > 0)
    }

    /// Records the plugins the host has disabled, once each, and answers whether any was new.
    fn note_disabled(&self) -> bool {
        let mut noted = lock(&self.disabled);
        let mut changed = false;
        for plugin in self.host.disabled() {
            if !noted.contains(&plugin) {
                noted.push(plugin);
                changed = true;
            }
        }
        changed
    }
}
