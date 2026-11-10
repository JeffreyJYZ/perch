//! The window under test: one app over a fixture, and the keys a test presses on it.

use std::time::{Duration, Instant};

use perch_app::{AppState, ProviderDefaults};
use perch_ui::Item;
use perch_ui::Shell;
use perch_ui::shell::composer_key;
use rune::{Key, KeyEvent, Mods, Tx};
use tempfile::TempDir;

/// How long a test waits for the turn to move.
pub const TIMEOUT: Duration = Duration::from_secs(10);

/// The provider the fixture server stands in for.
pub fn defaults(base_url: &str) -> ProviderDefaults {
    ProviderDefaults {
        base_url: base_url.to_string(),
        api_key: "test-key".to_string(),
        model: "test/model".to_string(),
    }
}

/// A window over an app with one session in a throwaway worktree.
///
/// Both temp directories outlive the app: the session log and the tool root are real paths until
/// the test ends.
pub fn window_against(fixture: &str) -> (Shell<AppState>, TempDir, TempDir) {
    let server = crate::support::serve(fixture);
    let data = tempfile::tempdir().unwrap();
    let worktree = tempfile::tempdir().unwrap();
    let mut app = AppState::open_with(data.path(), defaults(&server.base_url)).unwrap();
    let index = app.new_session(worktree.path()).unwrap();
    app.switch(index).unwrap();
    let sessions = app.metas();
    (Shell::new(sessions, 0).with_live(app), data, worktree)
}

/// One key press with no modifiers, as the window's map sees it.
pub fn press(shell: &mut Shell<AppState>, key: Key) -> bool {
    press_with(shell, key, Mods::default())
}

/// One key press with `mods`, as the window's map sees it.
pub fn press_with(shell: &mut Shell<AppState>, key: Key, mods: Mods) -> bool {
    composer_key(shell, &KeyEvent { key, mods }, &mut Tx::default())
}

/// One typed character.
pub fn type_key(shell: &mut Shell<AppState>, value: &str) -> bool {
    press_with(shell, Key::Text(value.to_string()), Mods::default())
}

/// Pumps the window until `done` holds, or [`TIMEOUT`] runs out.
pub fn pump_until(shell: &mut Shell<AppState>, done: impl Fn(&Shell<AppState>) -> bool) -> bool {
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        if done(shell) {
            return true;
        }
        shell.pump(Duration::from_millis(5));
    }
    done(shell)
}

/// How many turns the shown session has ended.
pub fn turns(shell: &Shell<AppState>) -> u32 {
    shell
        .live()
        .and_then(AppState::active_session)
        .map_or(0, |session| session.turns_ended())
}

/// Every assistant item's text, in order.
pub fn said(shell: &Shell<AppState>) -> Vec<String> {
    shell
        .transcript()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::Assistant { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// Every user item's text, in order.
pub fn asked(shell: &Shell<AppState>) -> Vec<String> {
    shell
        .transcript()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::User { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}
