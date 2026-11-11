//! perch-app: the session state the window is built from.
//!
//! [`AppState`] owns the concurrent half of the app — one agent, one dispatch thread and one
//! append-only log per session — and the transcript the window draws. Everything the dispatch
//! thread needs from a person arrives as a message, and the answer goes back on one channel, so
//! the window never blocks on a turn and a turn never reads the window's state directly.
//!
//! The tools are rebuilt rather than restarted: [`Sources`] builds a registry from the built-ins,
//! the plugin directory and the configured MCP servers, [`Live`] holds the one every dispatch
//! reads, and [`watch`] turns a save into a reload. The building happens on a worker thread, so a
//! server that is slow to answer costs the tools their seconds and never a frame.

pub mod answer;
pub mod app;
pub mod bridge;
pub mod launch;
mod live;
mod live_registry;
mod panels;
mod pump;
mod session;
pub mod settings;
mod sources;
mod turn;
pub mod update;
pub mod watch;
mod worker;

pub use app::{AppState, Error, ProviderDefaults, Result};
pub use bridge::{BridgeEvent, Reply};
pub use live_registry::Live;
pub use session::SessionRuntime;
pub use sources::Sources;
pub use turn::{Pending, Pumped};
