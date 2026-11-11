//! The project's own instructions travel in the system message the model is sent.
//!
//! A session must be told the rules of the repository it works in — the session's worktree, not the
//! data directory — and a repository with no instruction file must send the system message alone,
//! so a request from an app with nothing to add is byte-identical to today's.

#[path = "../../perch-agent/tests/support/mod.rs"]
mod support;

use std::time::{Duration, Instant};

use perch_app::{AppState, ProviderDefaults};
use tempfile::TempDir;

/// A recorded turn that answers at once, so one request reaches the fixture and nothing else.
const FIXTURE: &str = "tests/fixtures/live_text.txt";
/// How long the request is given to arrive. Bounded, not timed.
const TIMEOUT: Duration = Duration::from_secs(10);

fn defaults(base_url: &str) -> ProviderDefaults {
    ProviderDefaults {
        base_url: base_url.to_string(),
        api_key: "test-key".to_string(),
        model: "test/model".to_string(),
    }
}

/// The body of the first request a session in `worktree` makes.
fn first_request(instructions: Option<&str>) -> String {
    let server = support::serve(FIXTURE);
    support::hermetic();
    let data = TempDir::new().unwrap();
    let worktree = TempDir::new().unwrap();
    if let Some(text) = instructions {
        std::fs::write(worktree.path().join("AGENTS.md"), text).expect("the file is written");
    }
    let mut app =
        AppState::open_with(data.path(), defaults(&server.base_url)).expect("the app opens");
    let index = app
        .new_session(worktree.path())
        .expect("a session is added");
    app.switch(index).expect("the session is shown");
    app.prompt("hello").expect("the prompt is taken");

    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        if let Some(body) = server.bodies.lock().unwrap().first().cloned() {
            return body;
        }
        app.pump(Duration::from_millis(20));
    }
    panic!("no request reached the fixture within {TIMEOUT:?}");
}

#[test]
fn a_session_is_told_the_instructions_of_its_own_worktree() {
    let body = first_request(Some("Use tabs, never spaces."));
    assert!(
        body.contains("Use tabs, never spaces."),
        "the worktree's instructions are not in the request: {body}"
    );
}

#[test]
fn a_worktree_without_instructions_adds_nothing_to_the_system_message() {
    let body = first_request(None);
    assert!(
        !body.contains("Instructions from"),
        "a worktree with no instruction file still carried some: {body}"
    );
}
