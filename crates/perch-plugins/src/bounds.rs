//! What bounds one guest call: fuel, the epoch clock, the store's resource limits, and the lock a
//! plugin's roots take.
//!
//! These numbers belong to the host: a guest cannot raise them, a caller cannot pass them in, and
//! every call re-arms from here before it runs. A store that ran out of fuel on its last call
//! would otherwise either refuse the next one or run it unbounded, and both read as a plugin
//! fault rather than a bound.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use wasmtime::{Engine, Store, StoreLimitsBuilder};

use crate::abi::Data;

/// Instructions one guest call may burn before it is interrupted.
pub const CALL_FUEL: u64 = 50_000_000;
/// Wall-clock bound on one guest call, enforced by the epoch deadline below.
pub const CALL_TIME: Duration = Duration::from_secs(2);
/// How often the clock advances the engine's epoch.
const EPOCH_TICK: Duration = Duration::from_millis(50);
/// Epoch ticks a call may run, derived so it cannot drift from the wall-clock bound.
const EPOCH_TICKS: u64 = CALL_TIME.as_millis() as u64 / EPOCH_TICK.as_millis() as u64;
/// A guest never allocates more than this, however much it asks for.
const MEMORY_CAP: usize = 64 * 1024 * 1024;
/// Tables and instances one guest may create.
const INSTANCE_CAP: usize = 4;

/// Re-arms a store's bounds before a guest call, so every call starts with a full budget.
pub(crate) fn arm(store: &mut Store<Data>) -> wasmtime::Result<()> {
    store.set_fuel(CALL_FUEL)?;
    store.set_epoch_deadline(EPOCH_TICKS);
    Ok(())
}

/// The store one plugin's guest runs in: bounded memory, bounded instances, a deadline on every
/// call. The fuel itself is set by [`arm`], immediately before each call.
pub(crate) fn store(engine: &Engine) -> Store<Data> {
    let limits = StoreLimitsBuilder::new()
        .memory_size(MEMORY_CAP)
        .instances(INSTANCE_CAP)
        .tables(INSTANCE_CAP)
        .build();
    let mut store = Store::new(engine, Data::new(limits));
    store.limiter(|data| &mut data.limits);
    store.set_epoch_deadline(EPOCH_TICKS);
    store.epoch_deadline_trap();
    store
}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A panic while holding this lock would poison it; recovering keeps a broken plugin from
    // turning every later call into a panic of the host's own.
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Advances the engine's epoch so a store past its deadline is interrupted.
pub(crate) struct Clock {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Clock {
    /// A clock that never ticks: what a host with no engine holds, so the field needs no `Option`.
    pub(crate) fn idle() -> Self {
        Self {
            stop: Arc::new(AtomicBool::new(true)),
            thread: None,
        }
    }

    pub(crate) fn start(engine: &Engine) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let engine = engine.clone();
        let thread = std::thread::Builder::new()
            .name("perch-plugin-clock".to_string())
            .spawn(move || {
                while !flag.load(Ordering::Relaxed) {
                    std::thread::sleep(EPOCH_TICK);
                    engine.increment_epoch();
                }
            })
            .ok();
        Self { stop, thread }
    }
}

impl Drop for Clock {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
