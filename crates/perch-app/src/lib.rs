//! perch-app: the session state the window is built from.
//!
//! [`AppState`] owns the concurrent half of the app — one agent, one dispatch thread and one
//! append-only log per session — and the transcript the window draws. Everything the dispatch
//! thread needs from a person arrives as a message, and the answer goes back on one channel, so
//! the window never blocks on a turn and a turn never reads the window's state directly.

pub mod answer;
pub mod app;
pub mod bridge;

pub use app::{AppState, Error, Pending, ProviderDefaults, Result, SessionRuntime};
pub use bridge::{BridgeEvent, Reply};
