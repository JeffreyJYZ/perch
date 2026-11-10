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

#![deny(missing_docs)]

pub mod oauth;
mod remote;
mod stdio;
mod tools;

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use perch_store::settings::McpServerConfig;
use perch_tools::{ToolRegistry, ToolSpec};
use rmcp::model::Tool as DiscoveredTool;
use rmcp::service::{Peer, RoleClient, RunningService};
use serde_json::Value;

use tools::McpTool;

/// How long one MCP request may take: the handshake, `tools/list`, and every single
/// `tools/call`. A server that stopped answering therefore costs one call, never the app.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// How long a killed server is given to be seen as dead before the disconnect gives up
/// waiting. Only the test path needs it.
const DEATH_POLL_TIMEOUT: Duration = Duration::from_secs(5);

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
    /// A server was found dead and its tools were removed.
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

/// A connected server, as the host keeps it.
struct Server {
    name: String,
    peer: Peer<RoleClient>,
    /// Held only to keep the transport, and with it a stdio child, alive.
    _service: RunningService<RoleClient, ()>,
    /// The child's process id, for the liveness check and for killing it in tests.
    pid: Option<u32>,
    /// The namespaced names this server contributed.
    tools: Vec<String>,
}

impl Server {
    /// A server is alive while its transport is: the transport closes when a child exits or a
    /// remote session is lost, and that is the same event that makes every later call fail.
    fn is_alive(&self) -> bool {
        !self.peer.is_transport_closed()
    }

    /// Kills the child process outright and waits, bounded, until the client notices.
    ///
    /// `SIGKILL` rather than a shutdown handshake: the interesting case is a server that dies
    /// without saying goodbye, which is exactly what a crashed process looks like.
    fn kill(&mut self) {
        let Some(pid) = self.pid else {
            return;
        };
        // SAFETY: `pid` came from the child this host spawned and has not been reaped yet by
        // anything but its own transport, and signal 9 has no handler to run.
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGKILL);
        }
        let deadline = Instant::now() + DEATH_POLL_TIMEOUT;
        while Instant::now() < deadline && self.is_alive() {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// The parts of the host a tool needs: the runtime its calls run on, and the notes.
pub(crate) struct Shared {
    pub(crate) runtime: tokio::runtime::Runtime,
    notes: Mutex<Vec<Note>>,
}

impl Shared {
    fn note(&self, note: Note) {
        lock(&self.notes).push(note);
    }
}

/// Two MCP client halves: a peer to call through, and the running service that owns them.
pub(crate) struct Connected {
    pub(crate) peer: Peer<RoleClient>,
    pub(crate) service: RunningService<RoleClient, ()>,
    pub(crate) pid: Option<u32>,
}

/// Every configured MCP server, and the tools they contributed.
///
/// The host owns one Tokio runtime, because `rmcp` is async and the tool trait is not. Calls
/// enter it with `block_on` from the caller's own thread, which is why the host must not be
/// used from inside that runtime.
pub struct McpHost {
    shared: Arc<Shared>,
    servers: Mutex<Vec<Server>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl McpHost {
    /// Connect every configured server and register the tools each one offers.
    ///
    /// A server that fails is reported through [`McpHost::notes`] and skipped: one broken
    /// server must not stop the app from opening. Only an inability to build the runtime is an
    /// error, and that is a host-level fault rather than a server's.
    pub fn connect_all(configs: &[McpServerConfig], registry: &mut ToolRegistry) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()?;
        let host = Self {
            shared: Arc::new(Shared {
                runtime,
                notes: Mutex::new(Vec::new()),
            }),
            servers: Mutex::new(Vec::new()),
        };
        for config in configs {
            if let Err(error) = host.connect_one(config, registry) {
                host.shared.note(Note::Failed {
                    server: config.name.clone(),
                    error: error.to_string(),
                });
            }
        }
        Ok(host)
    }

    /// The names of the servers currently connected, in configuration order.
    pub fn server_names(&self) -> Vec<String> {
        lock(&self.servers)
            .iter()
            .map(|server| server.name.clone())
            .collect()
    }

    /// Whether the named server is still answering.
    pub fn is_alive(&self, name: &str) -> bool {
        lock(&self.servers)
            .iter()
            .any(|server| server.name == name && server.is_alive())
    }

    /// Everything the host has done, oldest first.
    pub fn notes(&self) -> Vec<Note> {
        lock(&self.shared.notes).clone()
    }

    /// The same notes as transcript lines.
    pub fn note_lines(&self) -> Vec<String> {
        self.notes().iter().map(Note::line).collect()
    }

    /// Drop every server that has died, unregistering its tools, and answer how many tools
    /// were removed.
    ///
    /// This is the path a crashed server takes: its tools leave the registry, so the model is
    /// never offered a call that cannot run.
    pub fn disconnect(&self, registry: &mut ToolRegistry) -> usize {
        let mut servers = lock(&self.servers);
        let dead: Vec<(String, Vec<String>)> = servers
            .iter()
            .filter(|server| !server.is_alive())
            .map(|server| (server.name.clone(), server.tools.clone()))
            .collect();
        if !dead.is_empty() {
            servers.retain(Server::is_alive);
        }
        drop(servers);

        let mut removed = 0;
        for (name, tools) in dead {
            let mut count = 0;
            for tool in &tools {
                if registry.unregister(tool) {
                    count += 1;
                }
            }
            removed += count;
            self.shared.note(Note::Removed {
                server: name,
                tools: count,
            });
        }
        removed
    }

    /// Kill the named server's process, for the test that has to prove a dead server is
    /// noticed. Nothing in the app calls this.
    pub fn kill_for_test(&self, name: &str) {
        let mut servers = lock(&self.servers);
        if let Some(server) = servers.iter_mut().find(|server| server.name == name) {
            server.kill();
        }
    }

    fn connect_one(&self, config: &McpServerConfig, registry: &mut ToolRegistry) -> Result<()> {
        let (connected, discovered) = self.open(config)?;
        let mut names = Vec::new();
        for tool in discovered {
            let name = separate(&config.name, &tool.name);
            let spec = ToolSpec {
                name: name.clone(),
                description: tool
                    .description
                    .as_deref()
                    .unwrap_or("tool contributed by an MCP server")
                    .to_string(),
                parameters: Value::Object((*tool.input_schema).clone()),
            };
            registry.register(Arc::new(McpTool {
                server: config.name.clone(),
                tool: tool.name.to_string(),
                spec,
                shared: Arc::clone(&self.shared),
                peer: connected.peer.clone(),
            }));
            names.push(name);
        }

        let count = names.len();
        lock(&self.servers).push(Server {
            name: config.name.clone(),
            peer: connected.peer,
            _service: connected.service,
            pid: connected.pid,
            tools: names,
        });
        self.shared.note(Note::Connected {
            server: config.name.clone(),
            tools: count,
        });
        Ok(())
    }

    /// Connect one server and list its tools, both bounded.
    ///
    /// The OAuth flow runs outside the request bound, because its bound is the one a human
    /// needs, not the one a machine needs; everything after it is a request and gets
    /// [`REQUEST_TIMEOUT`].
    fn open(&self, config: &McpServerConfig) -> Result<(Connected, Vec<DiscoveredTool>)> {
        let token = match (&config.command, &config.url) {
            (None, Some(url)) => match oauth::stored_token(&config.name)? {
                Some(token) => Some(token),
                None => Some(
                    self.shared
                        .runtime
                        .block_on(oauth::access_token(url, &config.name))?,
                ),
            },
            _ => None,
        };
        let shared = Arc::clone(&self.shared);
        let name = config.name.clone();
        shared.runtime.block_on(async {
            tokio::time::timeout(REQUEST_TIMEOUT, async {
                let connected = match (&config.command, &config.url) {
                    (Some(_), _) => stdio::connect(config).await?,
                    (None, Some(url)) => remote::connect(&name, url, token).await?,
                    (None, None) => {
                        return Err(Error::Config(format!(
                            "mcp server {} has neither a command nor a url",
                            config.name
                        )));
                    }
                };
                let tools =
                    connected
                        .peer
                        .list_all_tools()
                        .await
                        .map_err(|error| Error::Server {
                            server: name.clone(),
                            message: format!("tools/list failed: {error}"),
                        })?;
                Ok((connected, tools))
            })
            .await
            .map_err(|_| Error::Timeout {
                server: name.clone(),
                after: REQUEST_TIMEOUT,
            })?
        })
    }
}
