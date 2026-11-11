//! The live window's seams, driven headlessly.
//!
//! A window cannot be drawn in a test, but everything a window does can be: the key map is applied
//! to a real [`AppState`], the app's transcript is what the shell then draws, and every assertion
//! here is about what the app did — a turn sent, a call allowed, a question answered, a turn
//! stopped. The provider is the agent crate's own fixture server, so nothing reaches a network.
//!
//! What is *not* proved here: that the window paints. See `terminal.rs` for the dock's frames and
//! `tree.rs` for the laid-out tree.

#[path = "../../../perch-agent/tests/support/mod.rs"]
mod support;

mod approval;
mod folder;
mod harness;
mod keys;
mod missing;
mod question;
