//! perch-plugins: a wasm component host that registers plugin tools in the shared registry and
//! decodes the declarative panel each plugin declares.
//!
//! A plugin is a `wasm32-wasip2` component built against [`crates/perch-plugins/wit/perch-plugin.wit`].
//! It declares tools and runs them; the host owns every instance, bounds every call, disables a
//! plugin that traps and refuses a tool name another tool already holds. The ABI exposes no host
//! IO at all, so a plugin has nothing to read a file with — that refusal is the interface, not a
//! runtime check. Its panel is data ([`PanelModel`]) that the host draws itself, so a plugin has
//! no way to draw over the app's chrome.
//!
//! [`Loaded`] is the app's seam: one directory of `*.wasm`, the registry their tools registered
//! into, the panels they declare, the words for what did not load, and the event feed that carries
//! a session's events to them.

mod abi;
mod bounds;
mod feed;
mod host;
mod load;
mod loaded;
mod panel;
mod plugin;
mod tool;

pub use bounds::{CALL_FUEL, CALL_TIME};
pub use host::PluginHost;
pub use loaded::{Loaded, PanelEntry};
pub use panel::{PanelModel, PanelSection};
pub use tool::PluginTool;
