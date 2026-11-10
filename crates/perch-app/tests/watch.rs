//! The watcher on its own: which filesystem events mean "reload", and a real save producing one
//! signal. The rebuild and the swap are tested by driving `reload()` directly.

mod support;

use std::time::{Duration, Instant};

use notify::Event;
use perch_app::watch::{self, Watch};
use support::fixtures;

/// How long a real filesystem event is given to arrive. Bounded, not timed.
const TIMEOUT: Duration = Duration::from_secs(10);

fn event(kind: notify::EventKind, path: &str) -> Event {
    let mut event = Event::new(kind);
    event.paths = vec![path.into()];
    event
}

#[test]
fn a_write_to_the_settings_file_or_a_plugin_matters_and_a_read_does_not() {
    let settings = std::path::Path::new("/data/settings.toml");
    let plugins = std::path::Path::new("/data/plugins");
    let write = notify::EventKind::Modify(notify::event::ModifyKind::Any);
    let read = notify::EventKind::Access(notify::event::AccessKind::Any);

    assert!(watch::matters(
        &event(write, "/data/settings.toml"),
        settings,
        plugins
    ));
    assert!(watch::matters(
        &event(write, "/data/plugins/counter.wasm"),
        settings,
        plugins
    ));
    assert!(
        !watch::matters(&event(write, "/data/session.log"), settings, plugins),
        "a session log is not a settings or plugin change"
    );
    assert!(
        !watch::matters(&event(read, "/data/settings.toml"), settings, plugins),
        "reading a file is not a change to it"
    );
}

#[test]
fn a_file_dropped_into_the_plugin_directory_reports_one_change() {
    let data = tempfile::tempdir().unwrap();
    let plugins = data.path().join("plugins");
    std::fs::create_dir_all(&plugins).unwrap();
    let settings = perch_store::settings::path_in(data.path());
    std::fs::write(&settings, "").unwrap();

    let watcher = watch::watch(&settings, &plugins).expect("the watcher starts");
    std::fs::copy(
        fixtures().join("counter.wasm"),
        plugins.join("counter.wasm"),
    )
    .unwrap();

    assert!(wait(&watcher), "no change was reported within {TIMEOUT:?}");
}

fn wait(watcher: &Watch) -> bool {
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        if watcher.changed() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}
