//! Shared test support: a local server that replays recorded SSE bodies.
//!
//! Every file in `tests/` is its own crate, so this module is included rather than duplicated by
//! each test binary that needs the server.
//!
//! One fixture holds one response per request, in order. A turn that calls a tool asks again with
//! the result, so a recording of an agentic turn carries more than one response: `: next request`
//! on a line of its own starts the next. The whole body of every request is kept, which is how a
//! test reads the messages it was sent rather than trusting the shape of a struct.

#![allow(dead_code)]

mod recording;
mod request;
mod server;

// Each test binary needs a subset of this, so a name no binary in this crate uses is not an
// error — the crate is compiled once per `tests/` file.
#[allow(unused_imports)]
pub use server::{Fixture, fixture_server, serve};
