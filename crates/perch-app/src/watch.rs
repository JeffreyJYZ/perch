//! Watching `settings.toml` and the plugin directory, debounced, so a change is one reload.
//!
//! The watcher never touches the app: it turns a filesystem burst into one signal, and the app
//! that owns the registry decides when to rebuild. That keeps the reload on the thread that owns
//! the state, and it keeps a test able to drive the rebuild without a clock.
//!
//! A save is a burst — the editor writes a temporary file, renames it, the directory's metadata
//! changes — so a signal per event would rebuild several times for one save. The quiet gap is
//! what turns the burst into one reload.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::Duration;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};

/// How long the watcher waits for quiet before calling a burst one change.
pub const DEBOUNCE: Duration = Duration::from_millis(200);

/// A running watcher: `changed` is the signal the app drains, and dropping this stops it.
pub struct Watch {
    changed: Receiver<()>,
    /// Held for its `Drop`: the callback stops when the watcher goes.
    _watcher: RecommendedWatcher,
}

impl Watch {
    /// Whether a change has arrived since the last call.
    ///
    /// Non-blocking on purpose: this is asked once per frame, and a frame never waits on a file.
    pub fn changed(&self) -> bool {
        matches!(self.changed.try_recv(), Ok(()))
    }
}

/// Watches `settings` and `plugins`, answering `None` when the platform refuses to watch.
///
/// A watcher that will not start is not a reason to lose the window: the app keeps the tools it
/// opened with, and the next save is picked up the next time it starts.
pub fn watch(settings: &Path, plugins: &Path) -> Option<Watch> {
    // FSEvents reports real paths, and a temporary directory on macOS is `/var/...` whose real
    // path is `/private/var/...`. Comparing as given would miss every event, so both sides are
    // resolved once, here.
    let settings = resolve(settings);
    let plugins = resolve(plugins);
    let (raw_tx, raw_rx) = mpsc::channel::<()>();
    let watched_settings = settings.to_path_buf();
    let watched_plugins = plugins.to_path_buf();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
        let Ok(event) = event else {
            return;
        };
        if matters(&event, &watched_settings, &watched_plugins) {
            let _ = raw_tx.send(());
        }
    })
    .ok()?;
    for (path, mode) in watched_paths(&settings, &plugins) {
        // A directory that cannot be watched costs only that directory's changes.
        let _ = watcher.watch(&path, mode);
    }

    let (changed, incoming) = mpsc::channel();
    std::thread::Builder::new()
        .name("perch-watch".to_string())
        .spawn(move || {
            // One burst in, one signal out: wait for the events to stop for `DEBOUNCE`.
            while raw_rx.recv().is_ok() {
                loop {
                    match raw_rx.recv_timeout(DEBOUNCE) {
                        Ok(()) => {}
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                if changed.send(()).is_err() {
                    return;
                }
            }
        })
        .ok()?;

    Some(Watch {
        changed: incoming,
        _watcher: watcher,
    })
}

/// Whether `event` is a write to the settings file or anything under the plugin directory.
///
/// A read is not a change: opening the settings file, or a virus scanner walking the plugin
/// directory, must not cost a reload.
pub fn matters(event: &Event, settings: &Path, plugins: &Path) -> bool {
    if matches!(event.kind, EventKind::Access(_)) {
        return false;
    }
    event
        .paths
        .iter()
        .any(|path| path == settings || path.starts_with(plugins))
}

/// The real path of `path`, even when it does not exist yet: the parent is resolved and the last
/// component is put back, so a plugin directory created later still compares equal to its events.
fn resolve(path: &Path) -> PathBuf {
    if let Ok(real) = path.canonicalize() {
        return real;
    }
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => match parent.canonicalize() {
            Ok(real) => real.join(name),
            Err(_) => path.to_path_buf(),
        },
        _ => path.to_path_buf(),
    }
}

/// Which directories to watch, and how deeply.
/// The settings file's own directory is watched non-recursively, because a save replaces the file
/// rather than writing into an open handle. A plugin directory that does not exist yet is watched
/// through its parent, so the moment it is created is the moment its files start arriving.
fn watched_paths(settings: &Path, plugins: &Path) -> Vec<(PathBuf, RecursiveMode)> {
    let mut paths: Vec<(PathBuf, RecursiveMode)> = Vec::new();
    if let Some(dir) = settings.parent() {
        paths.push((dir.to_path_buf(), RecursiveMode::NonRecursive));
    }
    let (path, mode) = if plugins.is_dir() {
        (plugins.to_path_buf(), RecursiveMode::Recursive)
    } else {
        // Not created yet: the parent is watched deeply, so the directory appearing and the first
        // file written inside it are both seen.
        match plugins.parent() {
            Some(parent) => (parent.to_path_buf(), RecursiveMode::Recursive),
            None => return paths,
        }
    };
    if !paths.iter().any(|(watched, _)| *watched == path) {
        paths.push((path, mode));
    }
    paths
}
