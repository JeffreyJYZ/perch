//! Dropping servers that died, and the tools they contributed.
//!
//! A server is dropped for two reasons and the same code answers both: its transport closed, or
//! its connection never happened. Both leave an entry the host remembers, so a dead server is
//! visible to the one place that can remove it instead of only to a note nobody reads.

use perch_tools::ToolRegistry;

use crate::Note;
use crate::host::{McpHost, lock};

impl McpHost {
    /// Drop every server that has died or never connected, unregistering its tools, and answer how
    /// many tools were removed.
    ///
    /// This is the path a crashed server takes: its tools leave the registry, so the model is
    /// never offered a call that cannot run. A server that failed to connect had no tools, so it
    /// is dropped without a `Removed` line — its `Failed` note already said why.
    pub fn disconnect(&self, registry: &mut ToolRegistry) -> usize {
        let dead = {
            let mut servers = lock(&self.servers);
            let mut dead = Vec::new();
            let mut kept = Vec::new();
            for server in std::mem::take(&mut *servers) {
                if server.is_alive() {
                    kept.push(server);
                } else {
                    dead.push(server);
                }
            }
            *servers = kept;
            dead
        };

        let mut removed = 0;
        for mut server in dead {
            server.kill();
            let count = server.unregister(registry);
            removed += count;
            if count > 0 || server.error.is_none() {
                self.note(Note::Removed {
                    server: server.name,
                    tools: count,
                });
            }
        }
        removed
    }
}
