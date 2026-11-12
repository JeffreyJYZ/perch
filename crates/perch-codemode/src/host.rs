//! What a program may call: the injected tool surface, and the rule the approval gate rests on.

use perch_tools::{Result, ToolCtx, ToolOutput, ToolSpec};
use serde_json::Value;

/// The tools a program can reach, as the app sees them.
///
/// Shared behind an `Arc` and answering on `&self` because an `execute` call is itself one tool
/// call inside a registry: nothing here may need a mutable registry, and nothing here may resolve
/// back to the same `Arc` that holds the `execute` tool, or the two would never be freed.
pub trait ToolHost: Send + Sync {
    fn call(&self, name: &str, args: &Value, ctx: &ToolCtx) -> Result<ToolOutput>;

    fn specs(&self) -> Vec<ToolSpec>;

    /// Whether this tool asks a person before it runs.
    ///
    /// Required rather than defaulted: a host that answered `false` by omission would open the
    /// approval gate silently, and no default is safe for both a live registry and a test.
    fn gated(&self, name: &str) -> bool;
}

/// The words a refused call throws with.
pub fn refusal(name: &str) -> String {
    format!(
        "the `{name}` tool asks a person before it runs, so Code Mode cannot call it; call `{name}` directly so the change can be approved"
    )
}
