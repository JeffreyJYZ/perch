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

/// Tests must not read the machine they run on.
///
/// A real `~/.config/opencode/AGENTS.md` joins every session's system prompt, so the machine's own
/// notes would decide what the column says and a test would pass on one computer and fail on the
/// next. `PERCH_HOME` is the seam `perch_agent::instructions` documents for exactly this.
pub fn hermetic() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let home = std::env::temp_dir().join("perch-tests-home");
        let _ = std::fs::create_dir_all(&home);
        // Safety: set once per process, before any app is built, and only under test.
        unsafe { std::env::set_var("PERCH_HOME", &home) };
    });
}
