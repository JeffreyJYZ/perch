//! The two tools that have to sit at the end of a build, and the order between them.
//!
//! Both take a *copy* of the registry as it is when they register, so the copy is the whole of the
//! ordering: `task` is handed the registry as it was before Code Mode (a child can never run a
//! program), and Code Mode is handed the registry as it was before itself but after `task` (a
//! program can call a child). Registering either one first is a child that can run a program, or a
//! program that cannot delegate.

use std::sync::Arc;

use perch_codemode::{Execute, RegistryHost};
use perch_tools::ToolRegistry;

use crate::app::subagent::{self, Agents};

/// Adds Code Mode last, so a program can call every tool the build produced.
pub(super) fn add_codemode(registry: &mut ToolRegistry) {
    let host = Arc::new(RegistryHost::new(registry.clone()));
    registry.register(Arc::new(Execute::new(host)));
}

/// Adds `task`, answering the word to say when it cannot be offered — kept by the build while it
/// keeps failing, so a refusal is a line in the column rather than a tool that silently is not
/// there.
pub(super) fn add_task(registry: &mut ToolRegistry, agents: Option<&Agents>) -> Option<String> {
    subagent::register(registry, agents?)
}

/// Adds `webfetch` and `websearch`, the two tools that reach the network.
///
/// Registered before `task` and Code Mode, for the same reason `skill` is: a child turn and a
/// program are both places a page has to be read, and a registry copied before this point would
/// be one where neither could.
pub(super) fn add_web(registry: &mut ToolRegistry) {
    registry.register(Arc::new(perch_web::WebFetch::new()));
    registry.register(Arc::new(perch_web::WebSearch::new()));
}

/// Adds `skill`, which reads a `SKILL.md` from the machine or the project.
///
/// Registered before `task` and Code Mode so both are handed a registry that can already load a
/// skill: a child turn and a program are exactly the places a skill's instructions are needed.
pub(super) fn add_skills(registry: &mut ToolRegistry) {
    registry.register(Arc::new(perch_agent::SkillTool::new()));
}
