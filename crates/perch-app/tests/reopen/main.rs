//! A session reopened after a relaunch: its column comes back, and so does what the model knows
//! of it.
//!
//! Every test here opens an app over a log written by hand, so what is proved is that the log is
//! read — never that a provider was asked. The fixture server counts its requests and keeps every
//! body, which is what makes "opening a session sends nothing" a measurement and "the next request
//! carries the conversation" a reading of what went out rather than of a struct.

#[path = "../../../perch-agent/tests/support/mod.rs"]
mod support;

mod context;
mod harness;
mod opened;
mod transcript;
