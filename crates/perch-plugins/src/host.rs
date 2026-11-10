//! The plugin host: one wasmtime store per plugin, one registered tool per declared tool, and a
//! trap that disables the plugin instead of taking the app down with it.

use std::path::Path;

use perch_store::SessionEvent;
use perch_tools::{Error, Result, ToolRegistry};

use crate::bounds::{Clock, arm, lock};
use crate::load::{engine_and_linker, load_one, wasm_files};
use crate::panel::PanelModel;
use crate::plugin::{Plugin, PluginInner};

/// Every plugin loaded from one directory, the tools each contributed and the panel each
/// declares.
///
/// The host owns every wasmtime store: a [`PluginTool`](crate::PluginTool) reaches its guest
/// through a shared handle, so a trap disables a plugin with one flag. The registry owns its own
/// entries, so dropping a disabled plugin's tools is [`PluginHost::release`].
pub struct PluginHost {
    pub(crate) plugins: Vec<Plugin>,
    /// Keeps the engine's epoch moving, which is what bounds a call. Held for its `Drop`.
    _clock: Clock,
}

impl PluginHost {
    /// Loads every `*.wasm` in `dir`, in filename order, and registers each declared tool.
    ///
    /// A component that cannot be compiled or instantiated fails the whole load with its path in
    /// the message: a plugin the user installed must not disappear silently. A plugin that traps
    /// *while running* is disabled in place instead. Use [`PluginHost::load_all`] when one broken
    /// file should not cost the others.
    pub fn load(dir: &Path, registry: &mut ToolRegistry) -> Result<Self> {
        let (host, failures) = Self::load_all(dir, registry);
        match failures.into_iter().next() {
            Some(first) => Err(Error::Message(first)),
            None => Ok(host),
        }
    }

    /// The same load, keeping every plugin that loaded and answering one message per file that
    /// did not.
    ///
    /// This is what an app loading from a directory a person can drop a file into needs: a
    /// component that will not compile is one line of words, and the plugins beside it still run.
    /// The messages name the file, so the line is actionable without a log to search.
    pub fn load_all(dir: &Path, registry: &mut ToolRegistry) -> (Self, Vec<String>) {
        let mut files = match wasm_files(dir) {
            Ok(files) => files,
            Err(err) => return (Self::empty(), vec![format!("{}: {err}", dir.display())]),
        };
        files.sort();
        let (engine, linker) = match engine_and_linker() {
            Ok(pair) => pair,
            Err(err) => return (Self::empty(), vec![err.to_string()]),
        };
        let mut host = Self {
            plugins: Vec::new(),
            _clock: Clock::start(&engine),
        };
        let mut failures = Vec::new();
        for path in files {
            match load_one(&engine, &linker, &path, registry) {
                Ok(plugin) => host.plugins.push(plugin),
                Err(err) => failures.push(err.to_string()),
            }
        }
        (host, failures)
    }

    /// A host with no plugin: what a window with no plugin directory has.
    pub fn empty() -> Self {
        Self {
            plugins: Vec::new(),
            _clock: Clock::idle(),
        }
    }

    /// How many plugins are still loaded; a plugin disabled by a trap no longer counts.
    pub fn len(&self) -> usize {
        self.plugins
            .iter()
            .filter(|plugin| !plugin.is_disabled())
            .count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The tool names every live plugin contributed, in load order.
    pub fn names(&self) -> Vec<String> {
        self.plugins
            .iter()
            .filter(|plugin| !plugin.is_disabled())
            .flat_map(|plugin| plugin.tools.iter().cloned())
            .collect()
    }

    /// Every plugin's name, in load order, disabled ones included.
    ///
    /// A panel is addressed by plugin name, and a plugin that trapped still has one.
    pub fn plugin_names(&self) -> Vec<String> {
        self.plugins
            .iter()
            .map(|plugin| plugin.name.clone())
            .collect()
    }

    /// The plugins a trap has disabled, in load order.
    pub fn disabled(&self) -> Vec<String> {
        self.plugins
            .iter()
            .filter(|plugin| plugin.is_disabled())
            .map(|plugin| plugin.name.clone())
            .collect()
    }

    /// Unregisters every tool contributed by a plugin a trap disabled, answering how many names
    /// were removed.
    ///
    /// Removal cannot happen inside [`PluginTool::call`](crate::PluginTool): the registry owns its
    /// entries, and a tool is handed only its own arguments and the session root, never the
    /// registry. So the caller holding `&mut ToolRegistry` runs this after a call has failed —
    /// until then a disabled plugin's tools stay registered and answer the disabled error. Calling
    /// it again, or while no plugin is disabled, removes nothing and answers `0`.
    pub fn release(&self, registry: &mut ToolRegistry) -> usize {
        self.plugins
            .iter()
            .filter(|plugin| plugin.is_disabled())
            .flat_map(|plugin| plugin.tools.iter())
            .filter(|name| registry.unregister(name))
            .count()
    }

    /// The panel `plugin` declares, or `None` when it has none, is unknown, or has been disabled.
    ///
    /// The guest is asked every time, so the model is whatever it says now rather than a stale
    /// copy. A guest that faults while producing the model is disabled exactly as a trapping tool
    /// disables it — this is called on the UI's own thread, and a plugin must never be able to take
    /// the window down with it.
    pub fn panel_model(&self, plugin: &str) -> Option<PanelModel> {
        let loaded = self
            .plugins
            .iter()
            .find(|candidate| candidate.name == plugin && !candidate.is_disabled())?;

        let mut inner = lock(&loaded.inner);
        if inner.disabled {
            return None;
        }
        let outcome = {
            let PluginInner {
                store, bindings, ..
            } = &mut *inner;
            match arm(store) {
                Ok(()) => bindings.perch_plugin_panels().call_panel(store),
                Err(err) => Err(err),
            }
        };
        match outcome {
            Ok(model) => Some(PanelModel::from_wit(model)),
            Err(_) => {
                inner.disabled = true;
                None
            }
        }
    }

    /// Delivers one session event, as JSON, to every plugin still loaded.
    ///
    /// Every plugin is offered every event; a plugin that does not care decodes nothing and ignores
    /// it. A guest that faults here is disabled like any other trap, so one broken plugin cannot
    /// stop the others from seeing the event and cannot reach the app.
    pub fn dispatch_event(&mut self, event: &SessionEvent) {
        let Ok(event_json) = serde_json::to_string(event) else {
            return;
        };
        for plugin in &self.plugins {
            let mut inner = lock(&plugin.inner);
            if inner.disabled {
                continue;
            }
            let outcome = {
                let PluginInner {
                    store, bindings, ..
                } = &mut *inner;
                match arm(store) {
                    Ok(()) => bindings
                        .perch_plugin_panels()
                        .call_on_event(store, &event_json),
                    Err(err) => Err(err),
                }
            };
            if outcome.is_err() {
                inner.disabled = true;
            }
        }
    }
}
