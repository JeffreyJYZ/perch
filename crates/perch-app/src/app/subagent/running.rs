//! The session running each worktree, so a tool built before any session exists can still see the
//! turn that started it.
//!
//! The registry is shared by every session and built before the first one opens, so a worktree is
//! the only thing a call carries that names its caller. One session per checkout is the app's own
//! model, and a checkout opened again replaces its entry, so the table is bounded by the folders a
//! person opened rather than by turns.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use perch_agent::CancelWatch;

static RUNNING: OnceLock<Mutex<HashMap<PathBuf, CancelWatch>>> = OnceLock::new();

/// Records that `worktree` is now run by `watch`, so a child started there stops with it.
pub(crate) fn watching(worktree: &Path, watch: CancelWatch) {
    lock().insert(worktree.to_path_buf(), watch);
}

/// The cancel of the session that owns `worktree`, when one is open.
pub(super) fn watcher(worktree: &Path) -> Option<CancelWatch> {
    lock().get(worktree).cloned()
}

/// A lock that survives a thread that panicked while holding it: the table is plain data, and the
/// next session to open still has to be recorded.
fn lock() -> MutexGuard<'static, HashMap<PathBuf, CancelWatch>> {
    RUNNING
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
