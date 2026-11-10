//! A tool that arrives while a session is open is offered to that session's model.
//!
//! A plugin installed mid-session is callable at once through the live registry, but being
//! callable is not being offered: the model only calls what its request carried. This drives the
//! real app against a recorded provider and reads the request bodies, so the promise is checked
//! where the model would see it.

mod support;

#[path = "../../perch-agent/tests/support/mod.rs"]
mod fixture;

use std::time::Duration;

use perch_app::{AppState, ProviderDefaults};
use perch_store::secret::MemorySecrets;
use support::fixtures;

/// How long a turn is given. Bounded, not timed.
const TIMEOUT: Duration = Duration::from_secs(10);

/// The tool names the app offers right now.
fn names(app: &AppState) -> Vec<String> {
    app.tools().into_iter().map(|spec| spec.name).collect()
}

/// The body of request number `index`, counted from zero.
fn request(server: &fixture::Fixture, index: usize) -> String {
    let bodies = server.bodies.lock().unwrap();
    bodies
        .get(index)
        .unwrap_or_else(|| panic!("the fixture never received request {index}: {bodies:?}"))
        .clone()
}

#[test]
fn a_plugin_installed_mid_session_is_offered_on_the_next_request() {
    let server = fixture::serve("tests/fixtures/live_answers.txt");
    let data = tempfile::tempdir().unwrap();
    let worktree = tempfile::tempdir().unwrap();
    let mut app = AppState::open_with_secrets(
        data.path(),
        ProviderDefaults {
            base_url: server.base_url.clone(),
            api_key: "test-key".to_string(),
            model: "test/model".to_string(),
        },
        Box::new(MemorySecrets::default()),
    )
    .unwrap();
    let index = app.new_session(worktree.path()).unwrap();
    app.switch(index).unwrap();

    app.prompt("one").unwrap();
    assert!(
        app.pump_until(TIMEOUT, |session| session.turns_ended() == 1),
        "the first turn did not finish within {TIMEOUT:?}"
    );
    let first = request(&server, 0);
    assert!(
        !first.contains("\"name\":\"echo\""),
        "a tool that is not installed yet was offered: {first}"
    );

    // The plugin arrives with the session already running: no restart, no new session.
    let plugins = data.path().join(perch_app::settings::PLUGINS);
    std::fs::create_dir_all(&plugins).unwrap();
    std::fs::copy(
        fixtures().join("counter.wasm"),
        plugins.join("counter.wasm"),
    )
    .unwrap();
    assert!(app.reload(), "the reload could not read the settings file");
    assert!(
        names(&app).contains(&"echo".to_string()),
        "the app did not pick the plugin up: {:?}",
        names(&app)
    );

    app.prompt("two").unwrap();
    assert!(
        app.pump_until(TIMEOUT, |session| session.turns_ended() == 2),
        "the second turn did not finish within {TIMEOUT:?}"
    );
    let second = request(&server, 1);
    assert!(
        second.contains("\"name\":\"echo\""),
        "the next request does not offer the tool that arrived mid-session: {second}"
    );
}
