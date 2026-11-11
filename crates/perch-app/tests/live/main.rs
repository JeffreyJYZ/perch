//! The live session, end to end, against a local server that replays a recorded SSE body.
//!
//! Nothing here reaches a model or the network. Each test proves one promise of the window's
//! session state: a submitted prompt streams into the transcript and lands in the log, a call that
//! needs approval waits for the answer before it runs, its result goes back to the model for a
//! second request, and a cancel ends the turn exactly once.
//!
//! The fixture server is the agent crate's own, shared by path rather than copied, so the app
//! tests and the turn tests speak to the same recorder. It keeps every request body, which is how
//! a test proves what the model was sent rather than what a struct says.

#[path = "../../../perch-agent/tests/support/mod.rs"]
mod support;

mod approval;
mod cancel;
mod compact;
mod harness;
mod question;
mod queue;
mod stream;
mod subagent;
mod thinking;
