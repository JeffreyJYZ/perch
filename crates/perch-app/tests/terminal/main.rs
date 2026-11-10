//! The dock against a real shell.
//!
//! A window cannot be drawn in a test, but the two things the dock promises can be: a spawned
//! pty's output arrives as a [`Frame`](perch_term::Frame) and reaches the grid the dock paints,
//! and a shell that has exited says so in words instead of leaving its last frame on screen as if
//! it were still printing.
//!
//! `SHELL` is pointed at `/bin/sh` for this test binary: the user's own shell reads their rc, and
//! a test of the dock must not depend on what a dotfile prints.

mod exit;
mod harness;
mod spawn;
