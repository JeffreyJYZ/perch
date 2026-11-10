//! The frame's half: folding what the turn posted, and reading the tools the worker built.
//!
//! Nothing here waits on a rebuild. A frame folds the turn's events into the transcript, asks the
//! worker to hand the plugins the new log lines, and reads whatever snapshot the worker last
//! published — so a server that takes seconds to handshake costs the worker those seconds and
//! never a frame.

use std::sync::Arc;
use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant};

use crate::app::{AppState, QUIET};
use crate::turn::Pumped;
use crate::watch::Watch;

impl AppState {
    /// Folds what the dispatch thread has posted into the active transcript, waiting at most
    /// `budget` in total and at most [`QUIET`] between two events.
    ///
    /// A session waiting for the user stops the loop: no further event can arrive until
    /// [`AppState::answer`] is called, so the frame is the frame's.
    ///
    /// The watcher is serviced first, and never waited for: a change asks the worker for a
    /// rebuild and this frame carries on with the registry it already has. The snapshot that
    /// rebuild produces is picked up by a later frame, and [`AppState::busy`] keeps frames coming
    /// until it is.
    pub fn pump(&mut self, budget: Duration) -> Pumped {
        if self.watch.as_ref().is_some_and(Watch::changed) {
            self.reload_soon();
        }
        let mut pumped = self.pump_events(budget);
        if pumped.changed || self.turn_running() {
            self.feed_plugins();
        }
        if self.absorb() {
            pumped.changed = true;
        }
        if self.absorb_update() {
            pumped.changed = true;
        }
        pumped
    }

    /// Pumps until `done` holds, the session waits for the user, or `budget` runs out.
    ///
    /// The waiting case returns false rather than spinning: nothing can change until
    /// [`AppState::answer`] is called, so a caller that keeps pumping would only hold its frame.
    pub fn pump_until<F>(&mut self, budget: Duration, done: F) -> bool
    where
        F: Fn(&crate::SessionRuntime) -> bool,
    {
        let deadline = Instant::now() + budget;
        loop {
            match self.sessions.get(self.active) {
                Some(runtime) if done(runtime) => return true,
                Some(runtime) if runtime.is_waiting() => return false,
                Some(_) => {}
                None => return false,
            }
            if Instant::now() >= deadline {
                return false;
            }
            self.pump(QUIET);
        }
    }

    /// Whether a frame is owed: a turn is still arriving, or the worker still has work whose
    /// snapshot has not been read. A session waiting on the user does not count.
    pub fn busy(&self) -> bool {
        self.worker.busy()
            || self
                .update
                .as_ref()
                .is_some_and(crate::update::Worker::busy)
            || self
                .sessions
                .iter()
                .any(|session| session.is_running() && !session.is_waiting())
    }

    /// Whether the active session has a turn in flight.
    fn turn_running(&self) -> bool {
        self.sessions
            .get(self.active)
            .is_some_and(crate::SessionRuntime::is_running)
    }

    /// Hands the active session's new log lines to the plugins, without waiting for the answer.
    fn feed_plugins(&self) {
        if let Some(session) = self.sessions.get(self.active) {
            self.worker.feed(&self.dir, &session.meta);
        }
    }

    /// Makes the worker's newest snapshot the one the frame draws and every call dispatches
    /// through. Answers whether it was newer than the one already absorbed.
    pub(crate) fn absorb(&mut self) -> bool {
        let snapshot = self.worker.latest();
        if Arc::ptr_eq(&snapshot, &self.snapshot) {
            return false;
        }
        self.live.store(snapshot.registry.clone());
        self.snapshot = snapshot;
        true
    }

    /// Makes the update thread's newest status the one the column shows, answering whether it is
    /// newer than the one already drawn. Never waits: the status was published before this frame.
    fn absorb_update(&mut self) -> bool {
        let Some(worker) = self.update.as_ref() else {
            return false;
        };
        let latest = worker.latest();
        if self
            .update_status
            .as_ref()
            .is_some_and(|seen| Arc::ptr_eq(seen, &latest))
        {
            return false;
        }
        self.update_status = Some(latest);
        true
    }

    /// The event loop without the watcher and the plugin feed.
    fn pump_events(&mut self, budget: Duration) -> Pumped {
        let deadline = Instant::now() + budget;
        let mut pumped = Pumped::default();
        let Some(runtime) = self.sessions.get_mut(self.active) else {
            return pumped;
        };
        loop {
            // Everything already posted is folded even with a budget of zero: the budget is how
            // long a pump may *wait* for the next event, never how much of the queue it may leave
            // behind for the next frame.
            let event = match runtime.bridge.events.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Disconnected) => {
                    pumped.waiting = runtime.is_waiting();
                    return pumped;
                }
                Err(TryRecvError::Empty) => {
                    let now = Instant::now();
                    if now >= deadline {
                        pumped.waiting = runtime.is_waiting();
                        return pumped;
                    }
                    match runtime
                        .bridge
                        .events
                        .recv_timeout(QUIET.min(deadline - now))
                    {
                        Ok(event) => event,
                        // A quiet channel is a finished batch, and a disconnected one is a
                        // session whose thread has stopped. Neither is a reason to spin.
                        Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {
                            pumped.waiting = runtime.is_waiting();
                            return pumped;
                        }
                    }
                }
            };
            pumped.changed = true;
            if runtime.apply(event) {
                pumped.waiting = true;
                return pumped;
            }
        }
    }
}
