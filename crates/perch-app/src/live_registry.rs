//! The live tool registry: one list every session dispatches through, replaced as a whole.
//!
//! A reload builds a new registry from empty and swaps the pointer. Every dispatch takes the
//! current one when the call starts, so a call already running finishes against the tools it
//! began with, and the next call — in the same turn or a later one — sees the new list. No turn is
//! rewound, no message is edited, and nothing has to be told about a boundary.
//!
//! An [`ArcSwap`] rather than an `RwLock`: a reader only clones a pointer, so a dispatch never
//! waits for a rebuild, and a rebuild never waits for a call. The registry is a snapshot of
//! `Arc<dyn Tool>` entries, so cloning it is a list copy and the tools themselves stay shared.

use std::sync::Arc;

use arc_swap::ArcSwap;
use perch_tools::{Result, ToolCtx, ToolOutput, ToolRegistry, ToolSpec};

/// The app's one tool list, behind a pointer that can be replaced while calls are running.
#[derive(Clone)]
pub struct Live(Arc<ArcSwap<ToolRegistry>>);

impl Live {
    /// A live list holding `registry`.
    pub fn new(registry: ToolRegistry) -> Live {
        Live(Arc::new(ArcSwap::from_pointee(registry)))
    }

    /// Replaces the whole list. A dispatch already holding the old one keeps it until it returns.
    pub fn store(&self, registry: ToolRegistry) {
        self.0.store(Arc::new(registry));
    }

    /// The list as it is at this instant, held for as long as the caller keeps the guard.
    pub fn load(&self) -> arc_swap::Guard<Arc<ToolRegistry>> {
        self.0.load()
    }

    /// Every tool's spec, in registration order.
    pub fn specs(&self) -> Vec<ToolSpec> {
        self.load().specs()
    }

    /// Every tool's name, in registration order.
    pub fn names(&self) -> Vec<String> {
        self.specs().into_iter().map(|spec| spec.name).collect()
    }

    /// Dispatches through the list as it is now.
    pub fn call(&self, name: &str, args: &serde_json::Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        self.load().call(name, args, ctx)
    }
}
