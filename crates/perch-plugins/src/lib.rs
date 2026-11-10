//! perch-plugins: a wasm component host that registers plugin tools in the shared registry.
//!
//! A plugin is a `wasm32-wasip2` component built against [`crates/perch-plugins/wit/perch-plugin.wit`].
//! It declares tools and runs them; the host owns every instance, bounds every call, disables a
//! plugin that traps and refuses a tool name another tool already holds. The ABI exposes no host
//! IO at all, so a plugin has nothing to read a file with — that refusal is the interface, not a
//! runtime check.

mod abi;
mod host;

pub use host::{CALL_FUEL, CALL_TIME, PluginHost, PluginTool};
