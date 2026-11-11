//! Watching `settings.toml` and the plugin directory, debounced, so a change is one reload.
//!
//! The watcher never touches the app: it turns a filesystem burst into one signal, and the app
//! that owns the registry decides when to rebuild. That keeps the reload on the thread that owns
//! the state, and it keeps a test able to drive the rebuild without a clock.
//!
//! A save is a burst — the editor writes a temporary file, renames it, the directory's metadata
//! changes — so a signal per event would rebuild several times for one save. The quiet gap is
//! what turns the burst into one reload.
//!
//! The paths are polled rather than streamed. macOS's FSEvents backend blocks for one to fourteen
//! seconds per watched path while it installs a stream — measured, `docs/perf/startup.md` — and
//! during that window nothing is watched at all, so a save made just after the launch was lost.
//! Two paths holding one settings file and a handful of components are not a directory tree worth
//! a kernel stream: a `stat` sweep twice a second is the same guarantee, from the first frame.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, SystemTime};

use notify::{Config, Event, EventKind, PollWatcher, RecursiveMode, Watcher as _};

/// How long the watcher waits for quiet before calling a burst one change.
pub const DEBOUNCE: Duration = Duration::from_millis(200);

/// How often the watched paths are looked at. A settings save is a person's action, and the plugin
/// directory holds a handful of files, so the sweep is two metadata reads and a short walk.
pub const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// A running watcher: `changed` is the signal the app drains, and dropping this stops it.
pub struct Watch {
    changed: Receiver<()>,
    /// What the watched paths looked like when watching began.
    ///
    /// The poller records its baseline on its *first* sweep, so a save that lands before that
    /// sweep would be folded into the baseline and never reported — and that window is a real
    /// one: a person who opens the app and saves a setting straight away would lose the reload.
    /// Comparing against this snapshot closes exactly that window and nothing else.
    seed: Mutex<Seed>,
    /// Held for its `Drop`: the callback stops when the watcher goes.
    _watcher: PollWatcher,
}

/// The two paths whose change is a reload, and the instant each looked like when watching began.
struct Seed {
    settings: PathBuf,
    plugins: PathBuf,
    stamps: (Option<SystemTime>, Option<SystemTime>),
}

impl Seed {
    /// The stamps now, so a caller can compare and store in one step.
    fn now(&self) -> (Option<SystemTime>, Option<SystemTime>) {
        (stamp(&self.settings), stamp(&self.plugins))
    }
}

/// When `path` last changed, or `None` when it is not there yet.
fn stamp(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
}

impl Watch {
    /// Whether a change has arrived since the last call.
    ///
    /// Non-blocking on purpose: this is asked once per frame, and a frame never waits on a file.
    pub fn changed(&self) -> bool {
        if matches!(self.changed.try_recv(), Ok(())) {
            return true;
        }
        let mut seed = self.seed.lock().unwrap_or_else(|error| error.into_inner());
        let now = seed.now();
        if now != seed.stamps {
            seed.stamps = now;
            return true;
        }
        false
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
    let watched_settings = settings.clone();
    let watched_plugins = plugins.clone();
    let mut watcher = PollWatcher::new(
        move |event: notify::Result<Event>| {
            let Ok(event) = event else {
                return;
            };
            if matters(&event, &watched_settings, &watched_plugins) {
                let _ = raw_tx.send(());
            }
        },
        Config::default().with_poll_interval(POLL_INTERVAL),
    )
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

    let mut seed = Seed {
        settings,
        plugins,
        stamps: (None, None),
    };
    seed.stamps = seed.now();

    Some(Watch {
        changed: incoming,
        seed: Mutex::new(seed),
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
