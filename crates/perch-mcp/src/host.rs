//! The host: every configured server, the tools they contributed, and the reconciliation that
//! keeps the connected set matching the configuration.

use std::sync::{Arc, Mutex, MutexGuard};

use perch_store::settings::McpServerConfig;
use perch_tools::ToolRegistry;

use crate::server::Server;
use crate::{Note, Result};

/// The parts of the host a tool needs: the runtime its calls run on, and the notes.
pub(crate) struct Shared {
    pub(crate) runtime: tokio::runtime::Runtime,
    notes: Mutex<Vec<Note>>,
}

impl Shared {
    pub(crate) fn note(&self, note: Note) {
        lock(&self.notes).push(note);
    }
}

/// Every configured MCP server, and the tools they contributed.
///
/// The host owns one Tokio runtime, because `rmcp` is async and the tool trait is not. Calls
/// enter it with `block_on` from the caller's own thread, which is why the host must not be
/// used from inside that runtime.
///
/// A connection outlives any one registry: [`McpHost::reconcile`] connects, keeps or drops each
/// server as the configuration says, and [`McpHost::register_into`] fills a registry built from
/// empty with the tools the live connections already hold. Reloading the app's tools therefore
/// costs a server only when that server itself changed.
pub struct McpHost {
    pub(crate) shared: Arc<Shared>,
    pub(crate) servers: Mutex<Vec<Server>>,
}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
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
        host.reconcile(configs, registry);
        Ok(host)
    }

    /// Makes the connected set match `configs`, registering every live server's tools into a
    /// registry the caller built from empty.
    ///
    /// A server is kept only when its name *and* its configuration are unchanged and its
    /// transport is still open. Anything else is dropped — its child killed, its tools gone — and
    /// a configured name that is not connected is connected here, so this is the whole reload
    /// path: startup, a settings save, and a server that died all take it.
    pub fn reconcile(&self, configs: &[McpServerConfig], registry: &mut ToolRegistry) {
        let mut dropped = Vec::new();
        {
            let mut servers = lock(&self.servers);
            let mut kept = Vec::new();
            for server in std::mem::take(&mut *servers) {
                if server.is_alive() && configs.iter().any(|config| server.matches(config)) {
                    kept.push(server);
                } else {
                    dropped.push(server);
                }
            }
            *servers = kept;
        }
        for mut server in dropped {
            server.kill();
            let removed = server.unregister(registry);
            self.note(Note::Removed {
                server: server.name,
                tools: removed,
            });
        }

        let mut known: Vec<String> = lock(&self.servers)
            .iter()
            .map(|server| server.name.clone())
            .collect();
        for config in configs {
            if known.contains(&config.name) {
                continue;
            }
            // A name is counted as known whether it connected or not, so a duplicated entry costs
            // one attempt, not one process per copy.
            known.push(config.name.clone());
            if let Err(error) = self.connect_one(config) {
                let words = error.to_string();
                self.note(Note::Failed {
                    server: config.name.clone(),
                    error: words.clone(),
                });
                lock(&self.servers).push(Server::failed(config, words));
            }
        }
        self.register_into(registry);
    }

    /// Registers every live server's tools into `registry`, without touching a connection.
    ///
    /// This is what makes the registry a rebuilt snapshot rather than the connection's home: the
    /// tools are the same `Arc`s the server already made, so a fresh registry costs one pass.
    pub fn register_into(&self, registry: &mut ToolRegistry) {
        for server in lock(&self.servers)
            .iter()
            .filter(|server| server.is_alive())
        {
            for tool in &server.tools {
                registry.register(Arc::clone(tool) as Arc<dyn perch_tools::Tool>);
            }
        }
    }

    /// The names of the servers currently connected, in configuration order.
    pub fn server_names(&self) -> Vec<String> {
        lock(&self.servers)
            .iter()
            .filter(|server| server.is_alive())
            .map(|server| server.name.clone())
            .collect()
    }

    /// The configured servers that never connected, and what each said.
    ///
    /// A server that failed is remembered here rather than only in the notes, so a later
    /// reconciliation can drop it by name instead of trying it again forever.
    pub fn failed(&self) -> Vec<(String, String)> {
        lock(&self.servers)
            .iter()
            .filter_map(|server| {
                let error = server.error.clone()?;
                Some((server.name.clone(), error))
            })
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

    /// Kill the named server's process, for the test that has to prove a dead server is
    /// noticed. Nothing in the app calls this.
    pub fn kill_for_test(&self, name: &str) {
        let mut servers = lock(&self.servers);
        if let Some(server) = servers.iter_mut().find(|server| server.name == name) {
            server.kill();
        }
    }

    pub(crate) fn note(&self, note: Note) {
        self.shared.note(note);
    }
}
