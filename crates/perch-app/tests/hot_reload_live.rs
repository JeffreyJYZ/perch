//! What a running app picks up on its own: a plugin file dropped into the directory, and a
//! settings file naming a server that is no longer there.
//!
//! These are the two paths a frame drives — the watcher, its debounce and the swap for the plugin,
//! and a reload over a changed `mcp_servers` list for the server — so the test never calls the
//! watcher directly and never waits on a fixed clock.

mod support;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use perch_app::{AppState, ProviderDefaults};
use perch_store::Settings;
use perch_store::secret::MemorySecrets;
use perch_store::settings::McpServerConfig;
use perch_ui::shell::Panels;
use support::{fixtures, session};

/// How long a frame is given to notice the change. Bounded, not timed.
const TIMEOUT: Duration = Duration::from_secs(10);

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
    .expect("the app opens over the data directory")
}

fn plugin_dir(dir: &Path) -> PathBuf {
    dir.join(perch_app::settings::PLUGINS)
}

fn names(app: &AppState) -> Vec<String> {
    app.tools().into_iter().map(|spec| spec.name).collect()
}

#[test]
fn a_plugin_dropped_into_the_directory_is_picked_up_by_a_frame() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plugin_dir(dir.path())).unwrap();
    let mut app = app(dir.path());
    assert!(!names(&app).contains(&"echo".to_string()));

    std::fs::copy(
        fixtures().join("counter.wasm"),
        plugin_dir(dir.path()).join("counter.wasm"),
    )
    .unwrap();

    // Frames only: the watcher, its debounce and the swap, with no reload call of the test's own.
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        app.pump(Duration::from_millis(50));
        if names(&app).contains(&"echo".to_string()) {
            return;
        }
    }
    panic!(
        "no frame picked the plugin up within {TIMEOUT:?}: {:?}",
        names(&app)
    );
}

#[test]
fn a_server_that_never_connected_is_named_and_then_dropped() {
    let dir = tempfile::tempdir().unwrap();
    session(dir.path());
    let mut settings = Settings::load(dir.path()).unwrap();
    settings.mcp_servers = vec![McpServerConfig::stdio(
        "nowhere",
        "/nonexistent/perch-mcp-server",
        Vec::new(),
    )];
    settings.save(dir.path()).unwrap();

    let mut app = app(dir.path());
    assert!(
        app.mcp_servers().is_empty(),
        "a server that cannot start is reported as connected"
    );
    assert!(
        app.panels().is_empty()
            && app
                .plugin_words()
                .iter()
                .any(|words| words.contains("nowhere")),
        "the failure is not said anywhere: {:?}",
        app.plugin_words()
    );

    // Removing it from the configuration reconnects the set, which drops the failure too: a server
    // that was dead at startup used to be invisible to that reconciliation.
    let mut settings = Settings::load(dir.path()).unwrap();
    settings.mcp_servers.clear();
    settings.save(dir.path()).unwrap();
    app.reload();
    assert!(
        !app.plugin_words()
            .iter()
            .any(|words| words.contains("nowhere")),
        "the removed server is still named: {:?}",
        app.plugin_words()
    );
}
