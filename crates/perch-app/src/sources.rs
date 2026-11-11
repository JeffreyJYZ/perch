//! Everything a tool registry is built from: the built-ins, a directory of plugins and the
//! configured MCP servers.
//!
//! A rebuild starts from an empty registry and runs the sources in order, so the result depends
//! only on what is on disk and configured — never on what the last build happened to leave
//! behind. That is what makes a deleted plugin stay deleted: nothing survives that could
//! resurrect it.
//!
//! The MCP connections are the one thing that outlives a rebuild, because reconnecting a server
//! for a theme change would be spending a process on nothing. [`McpHost::reconcile`] keeps the
//! servers whose configuration did not change and connects or drops only the rest.

use std::path::{Path, PathBuf};

use perch_agent::Dispatcher;
use perch_mcp::McpHost;
use perch_plugins::{Loaded, PanelEntry};
use perch_store::SessionLog;
use perch_store::settings::McpServerConfig;
use perch_tools::ToolRegistry;

use crate::Result;
use crate::app::subagent::Agents;

mod build;

use build::{add_codemode, add_skills, add_task};

/// The live plugin host and MCP connections, and the registry they last built.
pub struct Sources {
    /// The directory a `*.wasm` is dropped into. A rebuild re-reads it from the start.
    dir: PathBuf,
    /// The provider a child `task` turn asks, when one was stated.
    ///
    /// Kept across a rebuild: a plugin or a server changing is not a reason for the child's
    /// provider to change, and the registry a person reloads is the same one they had.
    agents: Option<Agents>,
    loaded: Loaded,
    mcp: McpHost,
    /// The servers the last build could not connect, kept as words after the host has dropped the
    /// entries: a failure stays visible until the configuration changes, not until the next frame.
    failures: Vec<(String, String)>,
    /// What the last build that could not offer `task` said about it, kept for the same reason.
    refused: Option<String>,
    /// The words of the last press that reached no plugin, cleared by the next press that did.
    ///
    /// A press is answered by the worker's thread and no frame waits for it, so the refusal has
    /// one place to be said: the column, beside the plugin it was addressed to.
    action_error: Option<String>,
}

impl Sources {
    /// Loads the plugins in `dir` beside the built-ins, then connects every configured server.
    ///
    /// No provider is stated, so no child turn can be run and `task` is not offered: what a test
    /// or a script builds when it only wants to know what the plugins and servers produced.
    pub fn open(dir: PathBuf, servers: &[McpServerConfig]) -> Result<Sources> {
        Sources::open_for(dir, servers, None)
    }

    /// The same, with the provider a child `task` turn asks.
    pub(crate) fn open_for(
        dir: PathBuf,
        servers: &[McpServerConfig],
        agents: Option<Agents>,
    ) -> Result<Sources> {
        let mut loaded = Loaded::open(&dir, Dispatcher::builtin_registry());
        let mcp = McpHost::connect_all(servers, loaded.registry_mut())?;
        add_skills(loaded.registry_mut());
        let refused = add_task(loaded.registry_mut(), agents.as_ref());
        add_codemode(loaded.registry_mut());
        let failures = mcp.failed();
        Ok(Sources {
            dir,
            agents,
            loaded,
            mcp,
            failures,
            refused,
            action_error: None,
        })
    }

    /// Builds the registry again from empty: the built-ins, every plugin in `dir`, and every
    /// connected server. Answers the words the build produced.
    ///
    /// A plugin that fails to load is a line, not an abort: the plugins beside it, the built-ins
    /// and the MCP tools still land, which is exactly what a person installing one bad component
    /// needs.
    pub fn rebuild(&mut self, servers: &[McpServerConfig]) -> Vec<String> {
        let mut next = Loaded::open(&self.dir, Dispatcher::builtin_registry());
        // The new host starts where the old one stopped reading, so a reload does not replay a
        // session's whole log into a plugin that just started.
        next.inherit_feed(&mut self.loaded);
        self.mcp.reconcile(servers, next.registry_mut());
        add_skills(next.registry_mut());
        self.refused = add_task(next.registry_mut(), self.agents.as_ref());
        add_codemode(next.registry_mut());
        self.loaded = next;
        self.failures = self.mcp.failed();
        self.words()
    }

    /// The registry as the last build left it.
    pub fn registry(&self) -> &ToolRegistry {
        self.loaded.registry()
    }

    /// One line per file that did not load, per plugin a trap disabled, and per server the last
    /// build could not connect. Asked for every frame, because a trap can happen while a panel is
    /// drawn.
    pub fn words(&self) -> Vec<String> {
        let mut words = self.loaded.words();
        for (server, error) in &self.failures {
            words.push(format!("mcp {server} failed: {error}"));
        }
        if let Some(refused) = &self.refused {
            words.push(refused.clone());
        }
        if let Some(error) = &self.action_error {
            words.push(error.clone());
        }
        words
    }

    /// Reports a press on a control a plugin's panel declared, with the text an input was
    /// submitted with.
    ///
    /// The refusal is kept as words as well as returned, because the caller that presses is the
    /// frame and the caller that reads words is the column: a press for a plugin that has gone, or
    /// for an id it never declared, is said where a person can see it rather than swallowed by a
    /// frame that had nothing to draw it in.
    pub fn panel_action(
        &mut self,
        plugin: &str,
        control: &str,
        value: Option<&str>,
    ) -> std::result::Result<(), String> {
        match self.loaded.panel_action(plugin, control, value) {
            Ok(()) => {
                self.action_error = None;
                Ok(())
            }
            Err(message) => {
                self.action_error = Some(message.clone());
                Err(message)
            }
        }
    }

    /// The panels to draw, one per live plugin.
    pub fn panels(&self) -> Vec<PanelEntry> {
        self.loaded.panels()
    }

    /// Delivers the events `log` has grown by to the plugins, answering whether anything moved.
    pub fn sync(&mut self, log: &SessionLog) -> bool {
        self.loaded.sync(log)
    }

    /// Takes away the tools of a plugin that trapped and of an MCP server that died, answering
    /// whether the registry moved.
    ///
    /// A trap is noticed by the call that hit it, and a death by the transport closing, so the
    /// registry is corrected on the next frame rather than at the next restart.
    pub fn settle(&mut self) -> bool {
        let trapped = self.loaded.release();
        let died = self.mcp.disconnect(self.loaded.registry_mut());
        trapped + died > 0
    }

    /// How many plugins are live.
    pub fn plugin_count(&self) -> usize {
        self.loaded.len()
    }

    /// The names of the servers currently connected, in configuration order.
    pub fn mcp_servers(&self) -> Vec<String> {
        self.mcp.server_names()
    }

    /// The plugin directory being watched and rebuilt from.
    pub fn plugin_dir(&self) -> &Path {
        &self.dir
    }
}
