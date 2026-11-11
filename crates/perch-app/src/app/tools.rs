//! The app's tool list as the agent's: one snapshot taken at the start of every request.
//!
//! The two `ToolSpec` types are separate because the wire shape is the agent crate's, so this is
//! the only conversion — and it is what keeps the list the model is offered and the list a call
//! dispatches through the same list.

use perch_agent::ToolSpec;

use crate::live_registry::Live;

/// The live registry as the agent's tool source: a snapshot of `specs()` each time the agent
/// starts a request, so the next request carries whatever the last rebuild published, and a
/// request already streaming keeps the list it was sent with.
pub(super) fn live_source(live: &Live) -> impl Fn() -> Vec<ToolSpec> + Send + Sync + 'static {
    let live = live.clone();
    move || {
        live.specs()
            .into_iter()
            .map(|spec| ToolSpec {
                name: spec.name,
                description: spec.description,
                parameters: spec.parameters,
            })
            .collect()
    }
}
