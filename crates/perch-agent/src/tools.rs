//! Where a turn's tools come from, read fresh at the start of every request.
//!
//! The list is not read once when the agent starts. A plugin installed mid-session, or a server
//! that came back, is offered to the model on the very next request without a turn being
//! restarted — which is the difference between a tool that is merely callable and one the model
//! knows it may call.
//!
//! A change never edits a request that is already out. The list is a snapshot taken when the
//! request is built, so a reload that lands while a reply is streaming cannot alter what that
//! request carried.

use std::fmt;
use std::sync::Arc;

use crate::provider::ToolSpec;

/// The tool list a turn is offered: fixed for a test, read from the app's registry in a session.
#[derive(Clone)]
pub struct Tools(Arc<dyn Fn() -> Vec<ToolSpec> + Send + Sync>);

impl Tools {
    /// A list that never changes: what a scripted conversation and a test use.
    pub fn fixed(specs: Vec<ToolSpec>) -> Tools {
        Tools(Arc::new(move || specs.clone()))
    }

    /// A list read at the start of every request, from whatever owns the real one.
    pub fn live(source: impl Fn() -> Vec<ToolSpec> + Send + Sync + 'static) -> Tools {
        Tools(Arc::new(source))
    }

    /// The list as it is now. One call per request, and the answer is that request's tools for as
    /// long as it is being written and streamed.
    pub fn specs(&self) -> Vec<ToolSpec> {
        (self.0)()
    }
}

impl fmt::Debug for Tools {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Tools(<source>)")
    }
}
