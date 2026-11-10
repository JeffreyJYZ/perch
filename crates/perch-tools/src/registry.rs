//! The registry: tools by name, specs computed from what is registered, dispatch by name only.

use std::sync::Arc;

use crate::{Error, Result, Tool, ToolCtx, ToolOutput, ToolSpec, builtin::builtin_tools};

/// Holds every tool the model may call, built-in or contributed by a plugin.
///
/// Insertion order is kept so `specs()` is stable, and registering a name that already exists
/// replaces it in place instead of appending a duplicate.
#[derive(Default)]
pub struct ToolRegistry {
    tools: Vec<Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// An empty registry, which is what a test or a plugin-only host starts from.
    pub fn new() -> Self {
        Self::default()
    }

    /// The six shipped tools, in the order the model sees them.
    pub fn builtin() -> Self {
        let mut registry = Self::new();
        for tool in builtin_tools() {
            registry.register(tool);
        }
        registry
    }

    /// Adds a tool, replacing one of the same name so lookups stay single-valued.
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.spec().name;
        match self.tools.iter_mut().find(|t| t.spec().name == name) {
            Some(slot) => *slot = tool,
            None => self.tools.push(tool),
        }
    }

    /// Every registered tool's spec, in registration order.
    pub fn specs(&self) -> Vec<ToolSpec> {
        self.tools.iter().map(|tool| tool.spec()).collect()
    }

    /// Removes a tool by name, answering whether anything was removed.
    ///
    /// A plugin that traps has its tools dropped through here, so the model is never offered a
    /// spec whose tool cannot run.
    pub fn unregister(&mut self, name: &str) -> bool {
        let before = self.tools.len();
        self.tools.retain(|tool| tool.spec().name != name);
        self.tools.len() != before
    }

    /// Whether the named tool asks the user before it runs.
    pub fn needs_approval(&self, name: &str) -> Result<bool> {
        Ok(self.find(name)?.needs_approval())
    }

    /// Dispatches by name. No built-in gets a path a plugin could not take.
    pub fn call(&self, name: &str, args: &serde_json::Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        self.find(name)?.call(args, ctx)
    }

    fn find(&self, name: &str) -> Result<&Arc<dyn Tool>> {
        self.tools
            .iter()
            .find(|tool| tool.spec().name == name)
            .ok_or_else(|| Error::UnknownTool(name.to_string()))
    }
}
