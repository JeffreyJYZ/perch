//! An app over a fixture, and the readers a test uses on it.

use std::time::Duration;

use perch_app::{AppState, ProviderDefaults};
use perch_ui::Item;
use tempfile::TempDir;

/// How long a test waits for the bridge to deliver something it requires.
pub const TIMEOUT: Duration = Duration::from_secs(10);
/// How long a test listens for a second `TurnEnd` that must never arrive.
pub const SETTLE: Duration = Duration::from_millis(300);

/// The provider a fixture server stands in for.
pub fn defaults(base_url: &str) -> ProviderDefaults {
    ProviderDefaults {
        base_url: base_url.to_string(),
        api_key: "test-key".to_string(),
        model: "test/model".to_string(),
    }
}

/// An app with one session bound to a throwaway worktree, talking to `fixture`.
///
/// Both temp directories outlive the app, because the app's session log and the tool's root are
/// real paths until the test ends. The fixture comes back too, so a test can count its requests
/// and read the messages the model was sent.
pub fn app_against(fixture: &str) -> (AppState, crate::support::Fixture, TempDir, TempDir) {
    let server = crate::support::serve(fixture);
    let data = tempfile::tempdir().unwrap();
    let worktree = tempfile::tempdir().unwrap();
    let mut app = AppState::open_with(data.path(), defaults(&server.base_url)).unwrap();
    let index = app.new_session(worktree.path()).unwrap();
    app.switch(index).unwrap();
    (app, server, data, worktree)
}

/// The first tool card in the active transcript.
pub fn tool_card(app: &AppState) -> Option<&Item> {
    app.active_session()?
        .transcript()
        .items()
        .iter()
        .find(|item| matches!(item, Item::Tool { .. }))
}

/// Every assistant item's text, joined, as the column shows it.
pub fn said(app: &AppState) -> String {
    app.active_session()
        .unwrap()
        .transcript()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::Assistant { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// The body of request number `index`, counted from zero.
pub fn request(fixture: &crate::support::Fixture, index: usize) -> String {
    let bodies = fixture.bodies.lock().unwrap();
    bodies
        .get(index)
        .unwrap_or_else(|| panic!("the fixture never received request {index}: {bodies:?}"))
        .clone()
}
