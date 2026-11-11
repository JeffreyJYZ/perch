//! Temporary probe: does an explicit non-waiting rebuild land, and does the watcher see the file?

mod support;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use perch_app::{AppState, ProviderDefaults};
use perch_store::secret::MemorySecrets;
use perch_ui::shell::Panels;
use support::fixtures;

fn app(dir: &Path) -> AppState {
    AppState::open_with_secrets(
        dir,
        ProviderDefaults {
            base_url: "http://127.0.0.1:1/v1".to_string(),
            api_key: String::new(),
            model: "test/model".to_string(),
        },
        Box::new(MemorySecrets::default()),
    )
    .expect("the app opens")
}

fn names(app: &AppState) -> Vec<String> {
    app.tools().into_iter().map(|spec| spec.name).collect()
}

/// Pumps for up to `budget`, answering how long the profile took and whether echo arrived.
fn drive(app: &mut AppState, budget: Duration) -> (bool, Duration, usize) {
    let began = Instant::now();
    let mut pumps = 0;
    while began.elapsed() < budget {
        app.pump(Duration::from_millis(20));
        pumps += 1;
        if names(app).contains(&"echo".to_string()) {
            return (true, began.elapsed(), pumps);
        }
    }
    (false, began.elapsed(), pumps)
}

#[test]
fn probe_an_explicit_rebuild() {
    let dir = tempfile::tempdir().unwrap();
    support::session(dir.path());
    let plugins: PathBuf = dir.path().join(perch_app::settings::PLUGINS);
    std::fs::create_dir_all(&plugins).unwrap();
    let mut app = app(dir.path());
    std::fs::copy(
        fixtures().join("counter.wasm"),
        plugins.join("counter.wasm"),
    )
    .unwrap();
    let posted = app.reload_soon();
    let (landed, took, pumps) = drive(&mut app, Duration::from_secs(8));
    eprintln!("explicit: posted={posted} landed={landed} after {took:?} in {pumps} pumps");
    assert!(landed, "explicit rebuild never landed: {:?}", names(&app));
}

#[test]
fn probe_the_watcher() {
    let dir = tempfile::tempdir().unwrap();
    support::session(dir.path());
    let plugins: PathBuf = dir.path().join(perch_app::settings::PLUGINS);
    std::fs::create_dir_all(&plugins).unwrap();
    let mut app = app(dir.path());
    // The watcher installs its streams on its own thread; the app's own frames are what notice.
    std::fs::copy(
        fixtures().join("counter.wasm"),
        plugins.join("counter.wasm"),
    )
    .unwrap();
    let (landed, took, pumps) = drive(&mut app, Duration::from_secs(8));
    eprintln!("watcher: landed={landed} after {took:?} in {pumps} pumps");
    assert!(
        landed,
        "the watcher never fired: {:?} / {:?}",
        names(&app),
        app.plugin_words()
    );
}
