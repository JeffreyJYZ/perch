//! Who is calling: a name read when a call runs, not when the tool was built.
//!
//! One registry is built per process and every session shares it, so a tool cannot capture a
//! session at construction: the app swaps the active session between turns while the registry
//! stays put. Reading the name at call time is what makes `from` and the cursor follow the
//! session that is actually running.

use std::sync::{RwLock, RwLockWriteGuard};

/// The session a call belongs to, asked each time.
pub trait SessionName: Send + Sync {
    fn name(&self) -> String;
}

/// A name the app replaces when it switches sessions; the tools hold the same slot throughout.
#[derive(Debug, Clone, Default)]
pub struct Slot(std::sync::Arc<RwLock<String>>);

impl Slot {
    pub fn new(name: impl Into<String>) -> Self {
        Self(std::sync::Arc::new(RwLock::new(name.into())))
    }

    /// Points the slot at another session. The next call reads it; a call already running keeps
    /// the name it started with, so one read never mixes two sessions' cursors.
    pub fn set(&self, name: impl Into<String>) {
        *write(&self.0) = name.into();
    }
}

impl SessionName for Slot {
    fn name(&self) -> String {
        read(&self.0)
    }
}

/// A lock on its own, because the app may already own one: `Arc<RwLock<String>>` coerces to
/// `Arc<dyn SessionName>` with no wrapper.
impl SessionName for RwLock<String> {
    fn name(&self) -> String {
        read(self)
    }
}

/// A name that never changes, which is what the `&str` constructors hand in.
#[derive(Debug, Clone)]
pub struct Fixed(pub(crate) String);

impl SessionName for Fixed {
    fn name(&self) -> String {
        self.0.clone()
    }
}

/// A poisoned lock still holds the last name written, which is better than a blank one.
///
/// `into_inner` lives on `PoisonError`, not on the guard: the guard has no such function, so the
/// poison error is unwrapped into the guard it recovered.
fn read(lock: &RwLock<String>) -> String {
    lock.read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn write(lock: &RwLock<String>) -> RwLockWriteGuard<'_, String> {
    lock.write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
