//! How long a frame may spend, and how soon the window asks again.
//!
//! These are policy rather than plumbing, so they sit together instead of between the trait and
//! its methods: one is a budget a frame must never exceed, and the other two are the intervals a
//! window uses when there is no channel to wake it.

use std::time::Duration;

/// What one frame may spend folding a turn's events. The rest of the frame is the frame's: a
/// drawing thread that waits on a turn is a window that stops responding.
pub const BUDGET: Duration = Duration::from_millis(1);
/// How long the window waits before polling again while a turn is streaming.
pub const TURN_TICK: Duration = Duration::from_millis(8);
/// The same for a live shell, which has no channel to wake the window with: it is polled on a
/// tick because its output arrives whether or not anyone asked.
pub const SHELL_TICK: Duration = Duration::from_millis(16);
