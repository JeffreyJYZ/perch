//! Cancellation: the check a parked wait polls, so a cancelled turn cannot leave a tool running.
//!
//! The board does not own the turn, so it does not own the counter either. A host that has one
//! hands it in; a host with none gets [`NeverStopped`], which is what a plain test uses.

use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the turn that started a call has been cancelled.
pub trait Stopped: Send + Sync {
    fn stopped(&self) -> bool;
}

/// A check that never fires, for a host with nothing to cancel.
pub struct NeverStopped;

impl Stopped for NeverStopped {
    fn stopped(&self) -> bool {
        false
    }
}

/// A flag is the smallest live check: whoever cancels flips it, and the next poll sees it.
impl Stopped for AtomicBool {
    fn stopped(&self) -> bool {
        self.load(Ordering::SeqCst)
    }
}

/// A closure, so a host can hand over whatever its own cancellation already answers.
impl<F> Stopped for F
where
    F: Fn() -> bool + Send + Sync,
{
    fn stopped(&self) -> bool {
        self()
    }
}
