//! A rebuild that has to wait for a server waits on the worker, never on a frame.
//!
//! The slow part of a rebuild is a configured MCP server that has to be spawned and handshaked,
//! which is seconds. This configures one that takes two, drops a plugin in beside it, and drives
//! frames the way the window does: every frame must return at once, the tools must arrive only
//! after the server has finished failing, and the failure must be said.

mod support;

use std::time::{Duration, Instant};

use perch_app::{AppState, ProviderDefaults};
use perch_store::Settings;
use perch_store::secret::MemorySecrets;
use perch_store::settings::McpServerConfig;
use perch_ui::shell::Panels;
use support::fixtures;

/// How long the new tools are given to arrive. Bounded, not timed.
const TIMEOUT: Duration = Duration::from_secs(20);

/// How long the fake server takes to fail its handshake: `sh -c "sleep 2; exit 1"`.
const SLOW: Duration = Duration::from_millis(2000);

/// The tool names the app offers right now.
fn names(app: &AppState) -> Vec<String> {
    app.tools().into_iter().map(|spec| spec.name).collect()
}

#[test]
fn a_frame_pumps_while_a_slow_server_is_still_connecting() {
    let dir = tempfile::tempdir().unwrap();
    support::session(dir.path());
    let plugins = dir.path().join(perch_app::settings::PLUGINS);
    std::fs::create_dir_all(&plugins).unwrap();
    let mut app = AppState::open_with_secrets(
        dir.path(),
        ProviderDefaults {
            base_url: "http://127.0.0.1:1/v1".to_string(),
            api_key: String::new(),
            model: "test/model".to_string(),
        },
        Box::new(MemorySecrets::default()),
    )
    .unwrap();
    assert!(
        !names(&app).contains(&"echo".to_string()),
        "the plugin is offered before it was installed"
    );

    // Both arrive between two frames: a plugin to load, and a server that takes two seconds to
    // fail. A rebuild that ran on the frame thread would hold one frame for all of it.
    std::fs::copy(
        fixtures().join("counter.wasm"),
        plugins.join("counter.wasm"),
    )
    .unwrap();
    let mut settings = Settings::load(dir.path()).unwrap();
    settings.mcp_servers = vec![McpServerConfig::stdio(
        "slow",
        "/bin/sh",
        vec!["-c".to_string(), "sleep 2; exit 1".to_string()],
    )];
    settings.save(dir.path()).unwrap();

    let began = Instant::now();
    let mut slowest = Duration::ZERO;
    let mut arrived = None;
    while began.elapsed() < TIMEOUT {
        let frame = Instant::now();
        app.pump(Duration::from_millis(1));
        slowest = slowest.max(frame.elapsed());
        if names(&app).contains(&"echo".to_string()) {
            arrived = Some(began.elapsed());
            break;
        }
    }
    let arrived = arrived.unwrap_or_else(|| {
        panic!(
            "the new registry never arrived within {TIMEOUT:?}: {:?}",
            names(&app)
        )
    });

    assert!(
        arrived >= SLOW,
        "the registry arrived in {arrived:?}, before the server could have finished: \
         the rebuild did not wait for its connection"
    );
    assert!(
        slowest < perch_app::watch::DEBOUNCE,
        "a frame took {slowest:?} while a server was connecting; the debounce is \
         {:?}, so that frame stalled the window",
        perch_app::watch::DEBOUNCE
    );
    assert!(
        app.plugin_words()
            .iter()
            .any(|words| words.contains("slow")),
        "the server that never connected is not named: {:?}",
        app.plugin_words()
    );
    assert!(
        app.mcp_servers().is_empty(),
        "a server that never answered is reported as connected"
    );
}
