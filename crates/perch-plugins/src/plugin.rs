//! What the host keeps for one loaded plugin: its name, the tools it contributed, and the one
//! store every one of them reaches its guest through.
//!
//! One store per plugin is the whole isolation story: a trap sets one flag, and the tools, the
//! panel and the event handler of that plugin are all gone together.

use std::sync::{Arc, Mutex};

use wasmtime::Store;

use crate::abi::{self, Data};
use crate::bounds::lock;

/// One loaded plugin, as the host holds it.
pub(crate) struct Plugin {
    /// The wasm file's stem, which is the name the app addresses this plugin by.
    pub(crate) name: String,
    /// Every tool name this plugin registered, in declaration order.
    pub(crate) tools: Vec<String>,
    /// The store its tools and its panel share.
    pub(crate) inner: Arc<Mutex<PluginInner>>,
}

impl Plugin {
    pub(crate) fn is_disabled(&self) -> bool {
        lock(&self.inner).disabled
    }
}

/// The guest behind a plugin: its store, its bindings, and whether a trap has taken it.
pub(crate) struct PluginInner {
    pub(crate) store: Store<Data>,
    pub(crate) bindings: abi::PerchPlugin,
    /// Set by the first trap; every later call refuses instead of reaching the guest again.
    pub(crate) disabled: bool,
}
