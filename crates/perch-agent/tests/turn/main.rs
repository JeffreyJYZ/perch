//! Turn-loop tests against recorded SSE bodies served from `tests/fixtures`.
//!
//! A turn that calls a tool is not over: the loop asks again with the results in the conversation,
//! so every test here that sees a tool call also answers it, exactly as the app's dispatcher does.

#[path = "../support/mod.rs"]
mod support;

mod cancel;
mod harness;
mod rounds;
mod stream;
