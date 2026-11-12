//! The registry a child turn runs with, which is the whole of the recursion and write guard.

use perch_tools::{Error, Result, ToolRegistry};

use super::tool::TASK;

/// The parent's tools minus every tool that asks a person, and never `task` itself.
///
/// A tool whose approval answer cannot be read is dropped rather than kept: the safe direction for
/// a child nobody is watching is to reach less. A registry that already holds `task` is refused
/// outright, because the child would be handed the tool that lets it start a child of its own.
pub(super) fn child_tools(parent: &ToolRegistry) -> Result<ToolRegistry> {
    if parent.specs().iter().any(|spec| spec.name == TASK) {
        return Err(Error::Message(format!(
            "the registry already holds a tool named `{TASK}`, so a child given it could start its \
             own child; take the child's registry before `{TASK}` is registered"
        )));
    }
    let mut child = parent.clone();
    for spec in parent.specs() {
        if parent.needs_approval(&spec.name).unwrap_or(true) {
            child.unregister(&spec.name);
        }
    }
    Ok(child)
}
