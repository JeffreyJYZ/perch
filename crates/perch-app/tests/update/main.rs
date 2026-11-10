//! The updater wired into the app: the check off the frame thread, its one line, and the press
//! that stages an archive and never installs it.
//!
//! Nothing here reaches a network or a model: the release server is [`harness::Fake`], a table of
//! routes, and the app is opened over it the same way the window opens one.

mod check;
mod failures;
mod harness;
mod stage;
mod tree;
