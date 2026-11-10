//! perch-mcp: MCP servers as tool sources.
//!
//! One host owns every configured server. It spawns local ones and speaks MCP to them over
//! their pipes, reaches remote ones over streamable HTTP with OAuth, and turns every tool it
//! discovers into an ordinary [`perch_tools::Tool`]. Tools are registered under
//! `<server>__<tool>`, so no server can shadow a builtin or another server's tool.
//!
//! Nothing about an MCP call differs from a builtin call: the registry dispatches by name, the
//! call runs on the caller's thread against a bounded request, and its result is the same
//! [`perch_tools::ToolOutput`]. What is different is policy, and it is the user's call: an MCP
//! tool runs without an approval prompt, so instead every call is recorded — with the server
//! name — in [`McpHost::notes`], which is what the transcript reads.
//!
//! The host is reconciled rather than rebuilt: [`McpHost::reconcile`] compares the configured
//! list with the connected one and connects, keeps, or drops each server, so a settings change
//! costs only the servers it names. A registry built from empty is filled by
//! [`McpHost::register_into`] without reconnecting anything.

#![deny(missing_docs)]

mod connect;
mod disconnect;
mod host;
pub mod oauth;
mod remote;
mod server;
mod stdio;
mod tools;

use std::time::Duration;

pub(crate) use connect::Connected;
pub use host::McpHost;
pub(crate) use host::Shared;

/// How long one MCP request may take: the handshake, `tools/list`, and every single
/// `tools/call`. A server that stopped answering therefore costs one call, never the app.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// How long a killed server is given to be seen as dead before the disconnect gives up
/// waiting. Only the test path needs it.
pub(crate) const DEATH_POLL_TIMEOUT: Duration = Duration::from_secs(5);

/// The separator between a server name and a tool name.
pub const NAME_SEPARATOR: &str = "__";

/// `separate("fixture", "echo")` is `"fixture__echo"`.
///
/// Two hyphens and not a colon: a tool name has to survive being a JSON key and a match arm,
/// and a colon reads like a path separator in a transcript.
pub fn separate(server: &str, tool: &str) -> String {
    format!("{server}{NAME_SEPARATOR}{tool}")
}

/// Everything this crate can fail at.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A local process or a socket could not be started.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// A remote request could not be made.
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    /// A stored secret could not be read or written.
    #[error("store: {0}")]
    Store(#[from] perch_store::Error),
    /// A server was configured in a way that cannot be connected.
    #[error("{0}")]
    Config(String),
    /// A server did not finish a request inside [`REQUEST_TIMEOUT`].
    #[error("mcp server {server} did not answer within {after:?}")]
    Timeout {
        /// The server that stalled.
        server: String,
        /// The bound it overran.
        after: Duration,
    },
    /// A server answered, with an error.
    #[error("mcp server {server}: {message}")]
    Server {
        /// The server that failed.
        server: String,
        /// What it said.
        message: String,
    },
}

/// A result from this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// One line of what the host did, for the transcript.
///
/// The spec requires that MCP calls are recorded even though they are not approved first, so
/// the record carries the server name rather than only the namespaced tool name: a reader has
/// to be able to tell which process ran what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    /// A server connected and contributed `tools` tools.
    Connected {
        /// The configured server name.
        server: String,
        /// How many tools it offered.
        tools: usize,
    },
    /// A server could not be connected. Startup continues without its tools.
    Failed {
        /// The configured server name.
        server: String,
        /// Why it failed.
        error: String,
    },
    /// One call finished, successfully or not.
    Call {
        /// The server that served it.
        server: String,
        /// The tool name on that server, un-namespaced.
        tool: String,
        /// Whether the call returned a result rather than an error.
        ok: bool,
    },
    /// A server was found dead, or is no longer configured, and its tools were removed.
    Removed {
        /// The configured server name.
        server: String,
        /// How many tools were unregistered.
        tools: usize,
    },
}

impl Note {
    /// The transcript line for this note.
    pub fn line(&self) -> String {
        match self {
            Self::Connected { server, tools } => format!("mcp {server} connected: {tools} tools"),
            Self::Failed { server, error } => format!("mcp {server} failed: {error}"),
            Self::Call { server, tool, ok } => {
                let verdict = if *ok { "ok" } else { "failed" };
                format!("mcp {server}.{tool}: {verdict}")
            }
            Self::Removed { server, tools } => {
                format!("mcp {server} exited, {tools} tools removed")
            }
        }
    }
}
