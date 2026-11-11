//! What one execution may spend, and the token that stops it early.
//!
//! Every number is enforced independently and inside the host functions, never after the program
//! returns: `print` charges output bytes before it appends them and `tools.call` charges a call
//! before it runs one, so a program cannot print ten megabytes or make five hundred calls and
//! then be told no. The clock is checked at those same points; a program that computes without
//! ever reaching the host is bounded by the loop allowance in [`Limits::steps`], which boa's
//! runtime limits count for us.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::diag::{Hit, Stop};

/// A program is stopped after this long of wall clock.
pub const WALL: Duration = Duration::from_secs(30);
/// A program may call this many tools before it is stopped.
pub const CALLS: u32 = 32;
/// A program may produce this many bytes of output before the rest is dropped.
pub const OUTPUT: usize = 64 * 1024;
/// A program may run this many loop iterations without reaching a host call.
///
/// This is the clock's stand-in for code that never calls a tool: boa 0.22 counts loop
/// iterations and offers no interrupt hook, so a spin is bounded by count rather than by time.
/// The number is set so the spin ends in seconds even in a debug build, which is well past any
/// legitimate program's work and far under what a runaway one would cost.
pub const STEPS: u64 = 8_000_000;

/// Something an execution can ask whether it should stop.
///
/// A plain closure is one, so an app can hand over its own cancellation without naming a type:
/// `Arc::new(move || watch.cancelled())`.
pub trait Cancel: Send + Sync {
    fn cancelled(&self) -> bool;
}

impl<F: Fn() -> bool + Send + Sync> Cancel for F {
    fn cancelled(&self) -> bool {
        self()
    }
}

/// A token one thread sets and another reads.
#[derive(Debug, Clone, Default)]
pub struct Watch(Arc<AtomicBool>);

impl Watch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks the execution stopped; the next checkpoint answers [`Stop::Cancelled`].
    pub fn stop(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether the token has been set.
    pub fn stopped(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

impl Cancel for Watch {
    fn cancelled(&self) -> bool {
        self.stopped()
    }
}

/// The numbers one execution runs under, which a test may shrink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub wall: Duration,
    pub calls: u32,
    pub output: usize,
    pub steps: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            wall: WALL,
            calls: CALLS,
            output: OUTPUT,
            steps: STEPS,
        }
    }
}

/// What an execution has spent so far.
pub struct Budget {
    limits: Limits,
    started: Instant,
    calls: u32,
    produced: usize,
    cancel: Option<Arc<dyn Cancel>>,
}

impl Budget {
    pub fn start(limits: Limits, cancel: Option<Arc<dyn Cancel>>) -> Self {
        Self {
            limits,
            started: Instant::now(),
            calls: 0,
            produced: 0,
            cancel,
        }
    }

    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// The checkpoint: has the person stopped this, or has it run too long?
    pub fn poll(&mut self) -> Result<(), Stop> {
        if self
            .cancel
            .as_ref()
            .is_some_and(|cancel| cancel.cancelled())
        {
            return Err(Stop::Cancelled);
        }
        let after = self.started.elapsed();
        if after >= self.limits.wall {
            return Err(Stop::Hit(Hit::Clock {
                after,
                cap: self.limits.wall,
            }));
        }
        Ok(())
    }

    /// Charges one tool call, refusing the one that would go over.
    pub fn charge_call(&mut self) -> Result<(), Stop> {
        self.calls += 1;
        if self.calls > self.limits.calls {
            return Err(Stop::Hit(Hit::Calls {
                calls: self.limits.calls,
                cap: self.limits.calls,
            }));
        }
        Ok(())
    }

    /// Charges bytes a program produced, refusing the ones that would go over.
    pub fn charge_output(&mut self, bytes: usize) -> Result<(), Stop> {
        self.produced += bytes;
        if self.produced > self.limits.output {
            return Err(Stop::Hit(Hit::Output {
                bytes: self.produced,
                cap: self.limits.output,
            }));
        }
        Ok(())
    }

    /// The stop a loop allowance reports, so the tool never has to name a cause it cannot see.
    pub fn steps_hit(&self, steps: u64) -> Stop {
        Stop::Hit(Hit::Steps {
            steps,
            cap: self.limits.steps,
            wall: self.limits.wall,
        })
    }
}
