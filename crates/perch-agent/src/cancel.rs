//! Cancellation: a counter one thread bumps and another compares.
//!
//! A turn snapshots the counter when it starts and is cancelled the moment the counter it reads
//! is no longer that value, so a cancel cannot be lost by arriving between two checks, and a
//! cancel clicked while the agent was idle cannot kill the turn that starts afterwards.
//!
//! The turn thread publishes the generation it is running, so the thread that runs its tools can
//! ask the same question without owning either number. That thread runs a call after the model
//! asked for it, which may be after the user has already stopped the turn.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// The two numbers a cancel is decided from, shared by the turn thread and whoever reads it.
#[derive(Debug, Default)]
pub struct Cancel {
    /// Bumped by every [`AgentHandle::cancel`](crate::AgentHandle::cancel).
    nudges: AtomicU64,
    /// The generation of the turn that is running, published by the loop when it starts one.
    turn: AtomicU64,
}

impl Cancel {
    /// Take the generation this turn runs under, and publish it.
    ///
    /// Taken before the turn does anything, so a cancel from an earlier turn, or one made while
    /// the agent was idle, does not apply to this turn, and every later cancel does.
    pub fn begin(&self) -> u64 {
        let generation = self.nudges.load(Ordering::SeqCst);
        self.turn.store(generation, Ordering::SeqCst);
        generation
    }

    /// Whether the turn running as `generation` was cancelled.
    pub fn stopped(&self, generation: u64) -> bool {
        self.nudges.load(Ordering::SeqCst) != generation
    }

    /// Stop whatever turn is running.
    pub fn cancel(&self) {
        self.nudges.fetch_add(1, Ordering::SeqCst);
    }
}

/// A live view of one agent's cancellation, for the thread that runs its tools.
///
/// Nothing about a tool call makes it unwanted on its own: the turn that asked for it may have
/// been cancelled while the question was still travelling. This is how that thread asks before it
/// starts work, so a cancel the user pressed is not followed by a command they never wanted.
#[derive(Clone, Debug)]
pub struct CancelWatch(Arc<Cancel>);

impl CancelWatch {
    pub(crate) fn new(cancel: Arc<Cancel>) -> Self {
        Self(cancel)
    }

    /// Whether the turn that is running has been cancelled.
    #[must_use]
    pub fn cancelled(&self) -> bool {
        let running = self.0.turn.load(Ordering::SeqCst);
        self.0.stopped(running)
    }
}
