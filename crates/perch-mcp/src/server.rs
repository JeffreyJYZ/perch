//! One configured server, as the host keeps it: connected, or the reason it is not.
//!
//! A server that would not connect is still a server the host remembers. That is what makes it
//! possible to answer "why is this server not there" without a second connection attempt, and to
//! drop it when the configuration stops naming it instead of leaving it to be retried forever.

use std::sync::Arc;

use perch_store::settings::McpServerConfig;
use perch_tools::ToolRegistry;
use rmcp::service::{Peer, RoleClient, RunningService};

use crate::DEATH_POLL_TIMEOUT;
use crate::tools::McpTool;

/// Every server the host knows: one entry per name in the configuration, connected or failed.
pub(crate) struct Server {
    /// The configured name, which namespaces its tools.
    pub(crate) name: String,
    /// The configuration it was connected with, so a changed entry is a different server.
    pub(crate) config: McpServerConfig,
    /// The client half, when the connection was made.
    pub(crate) peer: Option<Peer<RoleClient>>,
    /// Held only to keep the transport, and with it a stdio child, alive.
    pub(crate) _service: Option<RunningService<RoleClient, ()>>,
    /// The child's process id, for the liveness check and for killing it in tests.
    pub(crate) pid: Option<u32>,
    /// The namespaced tools this server contributed.
    pub(crate) tools: Vec<Arc<McpTool>>,
    /// Why it never connected, when it did not.
    pub(crate) error: Option<String>,
}

impl Server {
    /// A server that connected and contributed `tools`.
    pub(crate) fn connected(
        config: &McpServerConfig,
        peer: Peer<RoleClient>,
        service: RunningService<RoleClient, ()>,
        pid: Option<u32>,
        tools: Vec<Arc<McpTool>>,
    ) -> Server {
        Server {
            name: config.name.clone(),
            config: config.clone(),
            peer: Some(peer),
            _service: Some(service),
            pid,
            tools,
            error: None,
        }
    }

    /// A server whose connection failed, kept so the column can say so and a reload can drop it.
    pub(crate) fn failed(config: &McpServerConfig, error: String) -> Server {
        Server {
            name: config.name.clone(),
            config: config.clone(),
            peer: None,
            _service: None,
            pid: None,
            tools: Vec::new(),
            error: Some(error),
        }
    }

    /// Whether this entry is the one `config` describes: same name, same arguments.
    pub(crate) fn matches(&self, config: &McpServerConfig) -> bool {
        self.name == config.name && self.config == *config
    }

    /// A server is alive while its transport is: the transport closes when a child exits or a
    /// remote session is lost, and that is the same event that makes every later call fail.
    pub(crate) fn is_alive(&self) -> bool {
        self.peer
            .as_ref()
            .is_some_and(|peer| !peer.is_transport_closed())
    }

    /// The namespaced names this server contributed.
    pub(crate) fn names(&self) -> Vec<String> {
        self.tools
            .iter()
            .map(|tool| tool.spec.name.clone())
            .collect()
    }

    /// Removes this server's tools from `registry`, answering how many names went.
    pub(crate) fn unregister(&self, registry: &mut ToolRegistry) -> usize {
        self.names()
            .iter()
            .filter(|name| registry.unregister(name))
            .count()
    }

    /// Kills the child process outright and waits, bounded, until the client notices.
    ///
    /// `SIGKILL` rather than a shutdown handshake: the interesting case is a server that dies
    /// without saying goodbye, which is exactly what a crashed process looks like.
    pub(crate) fn kill(&mut self) {
        let Some(pid) = self.pid else {
            return;
        };
        // SAFETY: `pid` came from the child this host spawned and has not been reaped yet by
        // anything but its own transport, and signal 9 has no handler to run.
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGKILL);
        }
        let deadline = std::time::Instant::now() + DEATH_POLL_TIMEOUT;
        while std::time::Instant::now() < deadline && self.is_alive() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}
