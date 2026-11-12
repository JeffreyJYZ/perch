//! An app whose first turn has a provider behind it, and the readers a title test uses.
//!
//! The worktree is a real `work` directory inside a throwaway root, so the default title is the
//! word a folder-name title would be and a rename is visibly a different one.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use perch_app::{AppState, ProviderDefaults};
use perch_files::Workspace;
use perch_ui::Shell;
use rune::{Env, Runtime};
use tempfile::TempDir;

/// How long a test waits for a turn or a title.
pub const TIMEOUT: Duration = Duration::from_secs(10);
/// How long a test listens for something that must not happen.
pub const SETTLE: Duration = Duration::from_millis(400);

/// The provider the fixture server stands in for.
pub fn defaults(base_url: &str) -> ProviderDefaults {
    ProviderDefaults {
        base_url: base_url.to_string(),
        api_key: "test-key".to_string(),
        model: "test/model".to_string(),
    }
}

/// An app with one session bound to `<root>/work`, talking to `fixture`. The root is returned
/// with it: the worktree is only a path while the directory that holds it is alive.
pub fn app_against(
    fixture: &str,
) -> (AppState, crate::support::Fixture, TempDir, TempDir, PathBuf) {
    let server = crate::support::serve(fixture);
    let data = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let work = root.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    // No machine-wide instructions: a real `AGENTS.md` would add a note to the column and decide
    // whether this test's "nothing was said about a title" assertion holds.
    crate::support::hermetic();
    let mut app = AppState::open_with(data.path(), defaults(&server.base_url)).unwrap();
    app.new_session(&work).unwrap();
    (app, server, data, root, work)
}

/// Sends one prompt and pumps until its turn ends.
pub fn turn(app: &mut AppState, text: &str) {
    let target = app
        .active_session()
        .map_or(1, |runtime| runtime.turns_ended() + 1);
    app.prompt(text).unwrap();
    assert!(
        app.pump_until(TIMEOUT, |runtime| runtime.turns_ended() >= target),
        "the turn should end within {TIMEOUT:?}"
    );
}

/// Pumps until the active session's title is `title`, or the budget runs out.
pub fn titled(app: &mut AppState, title: &str) -> bool {
    app.pump_until(TIMEOUT, |runtime| runtime.meta.title == title)
}

/// Pumps for `window` without asking whether anything arrived.
pub fn settle(app: &mut AppState, window: Duration) {
    let deadline = Instant::now() + window;
    while Instant::now() < deadline {
        app.pump(Duration::from_millis(5));
    }
}

/// The window over `app`, laid out once, so a test can read the sidebar's words.
pub fn laid_out(app: AppState) -> Runtime<Shell<AppState>> {
    let (metas, active) = (app.metas(), app.active());
    let shell = match metas
        .get(active)
        .and_then(|meta| Workspace::open(&meta.worktree).ok())
    {
        Some(workspace) => Shell::with_workspace(metas, active, workspace),
        None => Shell::new(metas, active),
    };
    let mut runtime = Runtime::new(
        shell.with_live(app),
        perch_ui::shell::view,
        (1280.0, 820.0),
        Env::new(true, 2.0),
    );
    runtime.on_key =
        Some(|shell: &mut Shell<AppState>, ev, tx| perch_ui::shell::window_key(shell, ev, tx));
    runtime.frame(0.0);
    runtime
}

/// Every word the left column says.
pub fn sidebar(runtime: &Runtime<Shell<AppState>>) -> String {
    let tree = runtime.accessibility_json();
    find(&tree, "left-column")
        .map(ToString::to_string)
        .unwrap_or_default()
}

fn find<'a>(node: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    if node["key"] == key {
        return Some(node);
    }
    node["children"]
        .as_array()?
        .iter()
        .find_map(|child| find(child, key))
}

/// The log's lines that are not the conversation, as text.
pub fn log_notes(app: &AppState) -> Vec<String> {
    app.active_session()
        .map(|session| session.log.events().unwrap_or_default())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|event| match event {
            perch_store::SessionEvent::Note { text } => Some(text),
            _ => None,
        })
        .collect()
}
