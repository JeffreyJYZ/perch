//! Opening the window: what each boot step costs, and the tools that arrive after it is open.
//!
//! Two things used to stand between the launch and the first frame, and both are measured here:
//! building the tool sources — a wasm engine per plugin and a handshake per MCP server — and
//! starting the file watcher, which on macOS cost one to fourteen seconds per watched path while
//! FSEvents installed a stream. Neither is done on the thread that opens the window any more: the
//! sources are built on the worker's thread and the paths are polled instead of streamed. The
//! tests prove the order rather than a duration, because a duration is a property of the machine.
//!
//! Run with `-- --nocapture` to read the numbers each step cost.

mod support;

use std::time::{Duration, Instant};

use perch_app::watch::{self, Watch};
use perch_app::{AppState, Sources};
use serde_json::json;
use support::{app, fixtures, session};

/// How long anything asynchronous here is given. Bounded, not timed.
const TIMEOUT: Duration = Duration::from_secs(20);

/// An app over a data directory whose `plugins/` holds the counter fixture.
fn opened() -> (tempfile::TempDir, AppState) {
    let data = tempfile::tempdir().unwrap();
    let plugins = data.path().join("plugins");
    std::fs::create_dir_all(&plugins).unwrap();
    std::fs::copy(
        fixtures().join("counter.wasm"),
        plugins.join("counter.wasm"),
    )
    .unwrap();
    session(data.path());
    let app = app(data.path());
    (data, app)
}

/// Whether the app offers a tool named `name` right now.
fn has(app: &AppState, name: &str) -> bool {
    app.tools().iter().any(|spec| spec.name == name)
}

/// Every tool name the app offers, for a failure that says what it did have.
fn names(app: &AppState) -> Vec<String> {
    app.tools().into_iter().map(|spec| spec.name).collect()
}

/// Pumps frames until `done` holds, or [`TIMEOUT`] runs out.
fn pump_until(app: &mut AppState, done: impl Fn(&AppState) -> bool) -> bool {
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        if done(app) {
            return true;
        }
        app.pump(Duration::from_millis(5));
    }
    done(app)
}

#[test]
fn the_window_opens_before_its_plugin_tools_do() {
    let (data, mut app) = opened();

    // The step the window no longer waits for, measured on this machine.
    let began = Instant::now();
    let sources =
        Sources::open(data.path().join("plugins"), &[]).expect("the fixture plugin loads");
    println!("Sources::open (the worker's thread): {:?}", began.elapsed());
    assert!(
        sources
            .registry()
            .specs()
            .iter()
            .any(|spec| spec.name == "echo"),
        "the fixture plugin did not load: {:?}",
        sources.words()
    );

    // The window has what a turn needs to start - the built-ins - and none of what the engine
    // builds. It was opened, not waited on: no frame has pumped, so the worker's snapshot has not
    // reached the live registry yet.
    assert!(
        has(&app, "read"),
        "a turn could not read: {:?}",
        names(&app)
    );
    assert!(
        !has(&app, "echo"),
        "the plugin's tool was in the window before the worker published it: {:?}",
        names(&app)
    );

    // A frame installs the snapshot, and the plugin's tools appear with it.
    assert!(
        pump_until(&mut app, |app| has(app, "echo")),
        "the plugin's tool never reached the window: {:?}",
        names(&app)
    );
}

#[test]
fn a_tool_that_has_not_arrived_is_named_in_the_refusal() {
    let (_data, app) = opened();

    // The model can call a tool whose plugin is still loading. Nothing panics on the list that
    // does not hold it yet: the call comes back naming the tool it could not find, which is what
    // the turn says instead of acting on nothing.
    let error = app
        .call_tool("echo", &json!({"text": "perch"}))
        .expect_err("the plugin's tool is not loaded yet")
        .to_string();
    assert!(
        error.contains("echo"),
        "the refusal does not name the tool: {error}"
    );
    println!("call before the tools arrive: {error}");
}

#[test]
fn the_file_watcher_starts_without_waiting_on_the_filesystem() {
    let (data, _app) = opened();
    let plugins = data.path().join("plugins");
    let settings = perch_store::settings::path_in(data.path());

    let began = Instant::now();
    let watcher = watch::watch(&settings, &plugins).expect("the watcher starts");
    let started = began.elapsed();
    println!("watch::watch returned after {started:?}");
    assert!(
        started < Duration::from_millis(100),
        "starting the watcher cost the window {started:?}"
    );

    // And it is watching from the first sweep: a save written now is one it reports.
    std::fs::write(&settings, "model = \"saved\"").unwrap();
    assert!(
        changed(&watcher),
        "no change was reported within {TIMEOUT:?}"
    );
}

/// Whether the watcher reports a change, waiting at most [`TIMEOUT`].
fn changed(watcher: &Watch) -> bool {
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        if watcher.changed() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}
