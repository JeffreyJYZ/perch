//! Reading a directory of `*.wasm` components into plugins.
//!
//! One file is one plugin, and loading is per file: a component that will not compile costs itself
//! and nothing else. What a broken file may not do is take the directory with it — a person who
//! installed three plugins and broke one still gets the other two, with the failure said in words.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use perch_tools::{Error, Result, ToolRegistry, ToolSpec};
use serde_json::json;
use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Config, Engine};

use crate::abi::{self, Data};
use crate::bounds::{arm, store};
use crate::plugin::{Plugin, PluginInner};
use crate::tool::PluginTool;

/// The engine every plugin runs in, and the linker that satisfies its imports.
///
/// Component model on, fuel on, the epoch clock on: the three switches the bounds module and the
/// ABI depend on. A failure here is the host's, not a plugin's, so it is one error and not one per
/// file.
pub(crate) fn engine_and_linker() -> Result<(Engine, Linker<Data>)> {
    let mut config = Config::new();
    config.consume_fuel(true);
    config.epoch_interruption(true);
    config.wasm_component_model(true);
    let engine =
        Engine::new(&config).map_err(|err| Error::Message(format!("plugin engine: {err}")))?;
    let mut linker: Linker<Data> = Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker)
        .map_err(|err| Error::Message(format!("wasi imports: {err}")))?;
    abi::PerchPlugin::add_to_linker::<Data, HasSelf<Data>>(&mut linker, |data: &mut Data| data)
        .map_err(|err| Error::Message(format!("plugin host imports: {err}")))?;
    Ok((engine, linker))
}

/// Every `*.wasm` in `dir`, unsorted: the caller loads them in its own order.
pub(crate) fn wasm_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("wasm") {
            files.push(path);
        }
    }
    Ok(files)
}

/// One component, compiled, instantiated and registered.
///
/// A declared name that is already taken — by a built-in or by an earlier plugin in the same
/// directory — fails the load with an error naming the plugin and the tool. Nothing is overwritten
/// and nothing is renamed: `registry.register` replaces in place, so registering a `bash` would
/// silently take the shipped tool away from the model, and `release` would never give it back
/// because it only removes names a disabled plugin owns. Every name is checked before a single
/// registration, so a colliding plugin contributes nothing at all.
pub(crate) fn load_one(
    engine: &Engine,
    linker: &Linker<Data>,
    path: &Path,
    registry: &mut ToolRegistry,
) -> Result<Plugin> {
    let plugin_name = plugin_name(path);
    let at = || path.display().to_string();

    let component = Component::from_file(engine, path)
        .map_err(|err| Error::Message(format!("{}: {err}", at())))?;
    let mut store = store(engine);
    arm(&mut store).map_err(|err| Error::Message(format!("{}: {err}", at())))?;
    let bindings = abi::PerchPlugin::instantiate(&mut store, &component, linker)
        .map_err(|err| Error::Message(format!("{}: {err}", at())))?;
    let specs = bindings
        .perch_plugin_tools()
        .call_tool_specs(&mut store)
        .map_err(|err| Error::Message(format!("{}: {err}", at())))?;

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

/// The plugin's name: its file's stem, which is what the app addresses it by.
fn plugin_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("plugin")
        .to_string()
}

/// The one error a name collision produces, naming the plugin and the tool it tried to take.
fn collision_error(path: &Path, plugin: &str, tool: &str) -> Error {
    Error::Message(format!(
        "{}: plugin `{plugin}` declares tool `{tool}`, which is already registered",
        path.display()
    ))
}
