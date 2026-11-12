//! The child half of `task`: a real agent turn, run on its own loop while the parent waits.
//!
//! The tool itself is `perch-agent`'s — it owns the caps, the child's registry and the refusal to
//! recurse. What it cannot own is a provider, so this is the delegate it is handed: one fresh
//! agent, the exact tools the tool computed, and the parent turn's cancellation.
//!
//! **The registry is built before any session exists**, so a child's parent cannot be captured
//! when the tool is built. It is found instead by the one thing a call carries: its worktree.
//! [`watching`] is where a session says which cancel that worktree runs under, and a child that
//! cannot find one runs under the tool's own — the shape a test wants, never the shape an app
//! does. Without it a cancel would leave the child working for nobody.
//!
//! The provider is the one the app was opened with. A key or a model saved afterwards reaches a
//! session's own turns, whose handles are updated in place, but not a child's: that is a second
//! handle, made once when the sources were built.

mod child;
mod running;

use std::sync::Arc;

use perch_agent::{Endpoint, Model, Task};
use perch_tools::ToolRegistry;

use super::ProviderDefaults;

pub(crate) use running::watching;

/// What every child is told it is: one paragraph, because a child starts with nothing else.
///
/// A child is not a session: it sees no conversation, its calls are not shown, and its answer is
/// all it contributes — so those three facts are what the paragraph has to carry.
const CHILD_SYSTEM: &str = "You are a child agent: perch started one turn of you for a single task, \
                             and you have nothing but the brief you were asked with. Use the tools \
                             you are given — every path is relative to the worktree — and answer \
                             with your final text, because nobody sees your tool calls and the \
                             answer is the whole of what comes back.";

/// The provider a child asks, and the paragraph every child is told it is.
#[derive(Clone)]
pub(crate) struct Agents {
    endpoint: Endpoint,
    model: Model,
    system: String,
}

impl Agents {
    /// The provider the app was opened with, which is the one a child asks.
    pub(crate) fn new(defaults: &ProviderDefaults) -> Agents {
        Agents {
            endpoint: Endpoint::new(defaults.base_url.clone(), defaults.api_key.clone()),
            model: Model::new(defaults.model.clone()),
            system: CHILD_SYSTEM.to_string(),
        }
    }
}

/// Adds `task` to `registry`, answering the word to say when it cannot be offered.
///
/// The child's registry is taken before this returns, so the tool a child would use to start a
/// child of its own never exists at all: `Task::new` refuses a registry that already holds one.
pub(crate) fn register(registry: &mut ToolRegistry, agents: &Agents) -> Option<String> {
    let agents = agents.clone();
    let built = Task::new(registry, move |tools| child::delegate(agents, tools));
    match built {
        Ok(task) => {
            registry.register(Arc::new(task));
            None
        }
        Err(error) => Some(format!("the `task` tool was not offered: {error}")),
    }
}
