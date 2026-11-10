//! The plugin host: one wasmtime store per plugin, one registered tool per declared tool, and a
//! trap that disables the plugin instead of taking the app down with it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use perch_store::SessionEvent;
use perch_tools::{Error, Result, Tool, ToolCtx, ToolOutput, ToolRegistry, ToolSpec};
use serde_json::{Value, json};
use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Config, Engine, Store, StoreLimitsBuilder};

use crate::abi::{self, Data};
use crate::panel::PanelModel;

/// Instructions one guest call may burn before it is interrupted.
pub const CALL_FUEL: u64 = 50_000_000;
/// Wall-clock bound on one guest call, enforced by the epoch deadline below.
pub const CALL_TIME: Duration = Duration::from_secs(2);
/// How often the clock advances the engine's epoch.
const EPOCH_TICK: Duration = Duration::from_millis(50);
/// Epoch ticks a call may run, derived so it cannot drift from the wall-clock bound.
const EPOCH_TICKS: u64 = CALL_TIME.as_millis() as u64 / EPOCH_TICK.as_millis() as u64;
/// A guest never allocates more than this, however much it asks for.
const MEMORY_CAP: usize = 64 * 1024 * 1024;
/// Tables and instances one guest may create.
const INSTANCE_CAP: usize = 4;

/// Every plugin loaded from one directory, the tools each contributed and the panel each
/// declares.
///
/// The host is the only owner of the wasmtime stores: a [`PluginTool`] reaches its guest through
/// a shared handle, so a trap disables a plugin with one flag. The registry still owns its own
/// entries, so dropping the tools of a disabled plugin is [`PluginHost::release`], which the
/// caller holding `&mut ToolRegistry` runs.
pub struct PluginHost {
    plugins: Vec<Plugin>,
    /// Keeps the engine's epoch moving, which is what bounds a call. Held for its `Drop`.
    _clock: Clock,
}

impl PluginHost {
    /// Loads every `*.wasm` in `dir`, in filename order, and registers each declared tool.
    ///
    /// A component that cannot be compiled or instantiated fails the whole load with its path in
    /// the message: a plugin the user installed must not disappear silently. A plugin that traps
    /// *while running* is a different case and is disabled in place.
    ///
    /// A declared name that is already taken — by a built-in or by an earlier plugin in the same
    /// directory — fails the load with an error naming the plugin and the tool. Nothing is
    /// overwritten and nothing is renamed: `registry.register` replaces in place, so registering a
    /// `bash` would silently take the shipped tool away from the model, and `release` would never
    /// give it back because it only removes names a disabled plugin owns. Rejection is per
    /// plugin, so a colliding plugin adds none of its tools while the plugins loaded before it
    /// keep theirs; the caller still holds the registry and can unregister by name.
    pub fn load(dir: &Path, registry: &mut ToolRegistry) -> Result<Self> {
        let mut wasm_files = wasm_files(dir)?;
        wasm_files.sort();

        let mut config = Config::new();
        config.consume_fuel(true);
        config.epoch_interruption(true);
        config.wasm_component_model(true);
        let engine =
            Engine::new(&config).map_err(|err| Error::Message(format!("plugin engine: {err}")))?;
        let clock = Clock::start(&engine);

        let mut linker: Linker<Data> = Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)
            .map_err(|err| Error::Message(format!("wasi imports: {err}")))?;
        abi::PerchPlugin::add_to_linker::<Data, HasSelf<Data>>(&mut linker, |data: &mut Data| data)
            .map_err(|err| Error::Message(format!("plugin host imports: {err}")))?;

        let mut plugins = Vec::new();
        for path in wasm_files {
            plugins.push(load_plugin(&engine, &linker, &path, registry)?);
        }

        Ok(Self {
            plugins,
            _clock: clock,
        })
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

    /// Unregisters every tool contributed by a plugin a trap disabled, answering how many names
    /// were removed.
    ///
    /// Removal cannot happen inside [`PluginTool::call`]: the registry owns its entries, and a
    /// tool is handed only its own arguments and the session root, never the registry. So the
    /// caller that holds `&mut ToolRegistry` runs this after a call has failed — until then a
    /// disabled plugin's tools stay registered and answer the disabled error. Calling it again,
    /// or calling it while no plugin is disabled, removes nothing and answers `0`; a plugin that
    /// is still live keeps its tools either way.
    pub fn release(&self, registry: &mut ToolRegistry) -> usize {
        self.plugins
            .iter()
            .filter(|plugin| plugin.is_disabled())
            .flat_map(|plugin| plugin.tools.iter())
            .filter(|name| registry.unregister(name))
            .count()
    }

    /// The panel `plugin` declares, or `None` when it has none, is unknown, or has been
    /// disabled.
    ///
    /// The guest is asked every time, so the model is whatever it says now rather than a stale
    /// copy. A guest that faults while producing the model is disabled exactly as a trapping
    /// tool disables it — this is called on the UI's own thread, and a plugin must never be able
    /// to take the window down with it.
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
    /// Every plugin is offered every event; a plugin that does not care decodes nothing and
    /// ignores it. A guest that faults here is disabled like any other trap, so one broken
    /// plugin cannot stop the others from seeing the event and cannot reach the app.
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

fn wasm_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("wasm") {
            files.push(path);
        }
    }
    Ok(files)
}

fn load_plugin(
    engine: &Engine,
    linker: &Linker<Data>,
    path: &Path,
    registry: &mut ToolRegistry,
) -> Result<Plugin> {
    let plugin_name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("plugin")
        .to_string();

    let component = Component::from_file(engine, path)
        .map_err(|err| Error::Message(format!("{}: {err}", path.display())))?;

    let limits = StoreLimitsBuilder::new()
        .memory_size(MEMORY_CAP)
        .instances(INSTANCE_CAP)
        .tables(INSTANCE_CAP)
        .build();
    let mut store = Store::new(engine, Data::new(limits));
    store.limiter(|data| &mut data.limits);
    store
        .set_fuel(CALL_FUEL)
        .map_err(|err| Error::Message(format!("{}: {err}", path.display())))?;
    store.set_epoch_deadline(EPOCH_TICKS);
    store.epoch_deadline_trap();

    let bindings = abi::PerchPlugin::instantiate(&mut store, &component, linker)
        .map_err(|err| Error::Message(format!("{}: {err}", path.display())))?;
    let specs = bindings
        .perch_plugin_tools()
        .call_tool_specs(&mut store)
        .map_err(|err| Error::Message(format!("{}: {err}", path.display())))?;

    // Names are checked before a single registration, so a plugin that collides contributes
    // nothing — not the tools that happened to come earlier in its spec list either. Every name
    // it declares must be free against what the registry already holds and against its own specs.
    let taken: Vec<String> = registry.specs().into_iter().map(|spec| spec.name).collect();
    let mut declared: Vec<String> = Vec::new();
    for spec in &specs {
        if taken.contains(&spec.name) || declared.contains(&spec.name) {
            return Err(collision_error(path, &plugin_name, &spec.name));
        }
        declared.push(spec.name.clone());
    }

    let inner = Arc::new(Mutex::new(PluginInner {
        store,
        bindings,
        disabled: false,
    }));

    let mut tools = Vec::new();
    for spec in specs {
        let parameters = serde_json::from_str(&spec.parameters_json)
            .unwrap_or_else(|_| json!({"type": "object", "properties": {}}));
        tools.push(spec.name.clone());
        registry.register(Arc::new(PluginTool {
            plugin: plugin_name.clone(),
            name: spec.name.clone(),
            spec: ToolSpec {
                name: spec.name,
                description: spec.description,
                parameters,
            },
            inner: Arc::clone(&inner),
        }));
    }

    Ok(Plugin {
        name: plugin_name,
        tools,
        inner,
    })
}

/// Re-arms a store's bounds before a guest call, so every call starts with a full budget.
///
/// Fuel and the epoch deadline are what bound a call; a store that ran out on the previous call
/// would otherwise refuse the next one, or run it unbounded.
fn arm(store: &mut Store<Data>) -> wasmtime::Result<()> {
    store.set_fuel(CALL_FUEL)?;
    store.set_epoch_deadline(EPOCH_TICKS);
    Ok(())
}

struct Plugin {
    /// The wasm file's stem, which is the name the app addresses this plugin by.
    name: String,
    tools: Vec<String>,
    inner: Arc<Mutex<PluginInner>>,
}

impl Plugin {
    fn is_disabled(&self) -> bool {
        lock(&self.inner).disabled
    }
}

struct PluginInner {
    store: Store<Data>,
    bindings: abi::PerchPlugin,
    disabled: bool,
}

/// One tool a plugin declared, registered through the same path a built-in takes.
pub struct PluginTool {
    plugin: String,
    name: String,
    spec: ToolSpec,
    inner: Arc<Mutex<PluginInner>>,
}

impl Tool for PluginTool {
    fn spec(&self) -> ToolSpec {
        // The real spec, disabled or not. A tool that cannot run is removed from the registry by
        // `PluginHost::release`, never hidden behind a spec with no name.
        self.spec.clone()
    }

    /// A plugin tool never needs approval: the ABI gives it no way to touch anything.
    fn needs_approval(&self) -> bool {
        false
    }

    fn call(&self, args: &Value, _ctx: &ToolCtx) -> Result<ToolOutput> {
        let args_json = serde_json::to_string(args)
            .map_err(|err| Error::Message(format!("arguments are not JSON: {err}")))?;

        let mut inner = lock(&self.inner);
        if inner.disabled {
            return Err(disabled_error(&self.plugin, &self.name));
        }

        let outcome = {
            let PluginInner {
                store, bindings, ..
            } = &mut *inner;
            store
                .set_fuel(CALL_FUEL)
                .map_err(|err| Error::Message(format!("plugin fuel: {err}")))?;
            store.set_epoch_deadline(EPOCH_TICKS);
            bindings
                .perch_plugin_tools()
                .call_call(store, &self.name, &args_json)
        };

        match outcome {
            Ok(Ok(text)) => Ok(ToolOutput::text(text)),
            Ok(Err(message)) => Err(Error::Message(message)),
            Err(err) => {
                // Fuel exhaustion, the epoch deadline and a wasm trap all land here. Disabling is
                // the whole point: a guest that faults must not be called again, and the error is
                // returned as text rather than unwinding through the caller.
                inner.disabled = true;
                Err(Error::Message(format!(
                    "plugin `{}` was disabled after `{}` trapped: {err}",
                    self.plugin, self.name
                )))
            }
        }
    }
}

fn disabled_error(plugin: &str, tool: &str) -> Error {
    Error::Message(format!(
        "plugin `{plugin}` is disabled, so `{tool}` is gone"
    ))
}

/// The one error a name collision produces, naming the plugin and the tool it tried to take.
fn collision_error(path: &Path, plugin: &str, tool: &str) -> Error {
    Error::Message(format!(
        "{}: plugin `{plugin}` declares tool `{tool}`, which is already registered",
        path.display()
    ))
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A panic while holding this lock would poison it; recovering keeps a broken plugin from
    // turning every later call into a panic of the host's own.
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Advances the engine's epoch so a store past its deadline is interrupted.
struct Clock {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Clock {
    fn start(engine: &Engine) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let engine = engine.clone();
        let thread = std::thread::Builder::new()
            .name("perch-plugin-clock".to_string())
            .spawn(move || {
                while !flag.load(Ordering::Relaxed) {
                    std::thread::sleep(EPOCH_TICK);
                    engine.increment_epoch();
                }
            })
            .ok();
        Self { stop, thread }
    }
}

impl Drop for Clock {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
