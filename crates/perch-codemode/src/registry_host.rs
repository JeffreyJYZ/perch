//! The host the app wires: a registry snapshot, with the approval rule in front of it.

use perch_tools::{Error, Result, ToolCtx, ToolOutput, ToolRegistry, ToolSpec};
use serde_json::Value;

use crate::host::{ToolHost, refusal};

/// Calls a snapshot of the app's registry, and never a tool a person must approve.
pub struct RegistryHost {
    registry: ToolRegistry,
}

impl RegistryHost {
    /// A host over `registry`.
    ///
    /// Take the snapshot *before* `execute` is registered on the registry the app dispatches
    /// from. A snapshot taken after would hold the `execute` tool, which holds this host, and an
    /// `Arc` loop like that is never freed — and it would also let a program call itself.
    pub fn new(registry: ToolRegistry) -> Self {
        Self { registry }
    }
}

impl ToolHost for RegistryHost {
    fn call(&self, name: &str, args: &Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        // The rule lives here as well as in the program's call path, so a host that is driven
        // directly still cannot reach a tool that asks a person first.
        if self.gated(name) {
            return Err(Error::Message(refusal(name)));
        }
        self.registry.call(name, args, ctx)
    }

    fn specs(&self) -> Vec<ToolSpec> {
        self.registry.specs()
    }

    fn gated(&self, name: &str) -> bool {
        // An unknown tool is not gated, so the call answers "no tool named …" rather than a
        // refusal about approval it would never have needed.
        self.registry.needs_approval(name).unwrap_or(false)
    }
}
