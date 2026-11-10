//! The live window's seams, driven headlessly.
//!
//! A window cannot be drawn in a test, but everything a window does can be: the key map is applied
//! to a real [`AppState`], the app's transcript is what the shell then draws, and every assertion
//! here is about what the app did — a turn sent, a call allowed, a question answered, a turn
//! stopped. The provider is the agent crate's own fixture server, so nothing reaches a network.
//!
//! What is *not* proved here: that the window paints. See `terminal.rs` for the dock's frames and
//! `tree.rs` for the laid-out tree.

#[path = "../../perch-agent/tests/support/mod.rs"]
mod support;

use std::time::{Duration, Instant};

use perch_app::{AppState, ProviderDefaults};
use perch_store::Settings;
use perch_store::settings::McpServerConfig;
use perch_ui::shell::{Waiting, composer_key};
use perch_ui::{Item, Shell};
use rune::{Env, Key, KeyEvent, Mods, Runtime, Tx};
use tempfile::TempDir;

/// How long a test waits for the turn to move.
const TIMEOUT: Duration = Duration::from_secs(10);

/// The provider the fixture server stands in for.
fn defaults(base_url: &str) -> ProviderDefaults {
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
fn window_against(fixture: &str) -> (Shell<AppState>, TempDir, TempDir) {
    let server = support::serve(fixture);
    let data = tempfile::tempdir().unwrap();
    let worktree = tempfile::tempdir().unwrap();
    let mut app = AppState::open_with(data.path(), defaults(&server.base_url)).unwrap();
    let index = app.new_session(worktree.path()).unwrap();
    app.switch(index).unwrap();
    let sessions = app.metas();
    (Shell::new(sessions, 0).with_live(app), data, worktree)
}

/// One key press with no modifiers, as the window's map sees it.
fn press(shell: &mut Shell<AppState>, key: Key) -> bool {
    press_with(shell, key, Mods::default())
}

/// One key press with `mods`, as the window's map sees it.
fn press_with(shell: &mut Shell<AppState>, key: Key, mods: Mods) -> bool {
    composer_key(shell, &KeyEvent { key, mods }, &mut Tx::default())
}

/// One typed character.
fn type_key(shell: &mut Shell<AppState>, value: &str) -> bool {
    press_with(shell, Key::Text(value.to_string()), Mods::default())
}

/// Pumps the window until `done` holds, or [`TIMEOUT`] runs out.
fn pump_until(shell: &mut Shell<AppState>, done: impl Fn(&Shell<AppState>) -> bool) -> bool {
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
fn turns(shell: &Shell<AppState>) -> u32 {
    shell
        .live()
        .and_then(AppState::active_session)
        .map_or(0, |session| session.turns_ended())
}

/// Every assistant item's text, in order.
fn said(shell: &Shell<AppState>) -> Vec<String> {
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
fn asked(shell: &Shell<AppState>) -> Vec<String> {
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

#[test]
fn enter_sends_the_composer_as_one_turn_and_the_app_owns_it() {
    let (mut shell, _data, _worktree) = window_against("tests/fixtures/live_text.txt");
    shell.set_prompt("what is this?");

    assert!(press(&mut shell, Key::Enter), "Enter is the send key");
    assert_eq!(
        shell.prompt_text(),
        "",
        "the box empties once the app took the text"
    );
    assert!(
        pump_until(&mut shell, |shell| turns(shell) == 1),
        "the turn should end within {TIMEOUT:?}"
    );

    assert_eq!(
        asked(&shell),
        ["what is this?".to_string()],
        "the app adds the user turn, and adds it once"
    );
    assert!(
        said(&shell)
            .iter()
            .any(|text| text.contains("Hello from the fixture")),
        "the reply streams into the column: {:?}",
        said(&shell)
    );

    let log = shell
        .live()
        .expect("the window kept its app")
        .active_session()
        .expect("the session is open")
        .log
        .events()
        .unwrap();
    assert_eq!(
        log.iter()
            .filter(|event| matches!(event, perch_store::SessionEvent::User { .. }))
            .count(),
        1,
        "the question is in the log exactly once: {log:?}"
    );
}

#[test]
fn enter_allows_the_call_that_is_waiting() {
    let (mut shell, _data, worktree) = window_against("tests/fixtures/live_write.txt");
    shell.set_prompt("write made.txt");
    press(&mut shell, Key::Enter);

    assert!(
        pump_until(&mut shell, |shell| shell.waiting() == Waiting::Approval),
        "the write should ask for approval within {TIMEOUT:?}"
    );
    assert!(
        !worktree.path().join("made.txt").exists(),
        "the call has not run while it waits"
    );

    assert!(
        press(&mut shell, Key::Enter),
        "Enter allows the waiting call"
    );
    assert!(
        pump_until(&mut shell, |shell| turns(shell) == 1),
        "the turn should finish once the call is allowed"
    );
    assert_eq!(
        std::fs::read_to_string(worktree.path().join("made.txt")).unwrap(),
        "hello from perch",
        "the allowed call ran"
    );
}

#[test]
fn escape_refuses_the_call_that_is_waiting() {
    let (mut shell, _data, worktree) = window_against("tests/fixtures/live_write.txt");
    shell.set_prompt("write made.txt");
    press(&mut shell, Key::Enter);

    assert!(
        pump_until(&mut shell, |shell| shell.waiting() == Waiting::Approval),
        "the write should ask for approval within {TIMEOUT:?}"
    );
    assert!(press(&mut shell, Key::Escape), "Escape refuses the call");
    assert!(
        pump_until(&mut shell, |shell| turns(shell) == 1),
        "a refusal still ends the turn"
    );
    assert!(
        !worktree.path().join("made.txt").exists(),
        "a refused call does not run"
    );
}

#[test]
fn a_number_key_picks_and_enter_confirms() {
    let (mut shell, _data, _worktree) = window_against("tests/fixtures/live_question.txt");
    shell.set_prompt("go on");
    press(&mut shell, Key::Enter);

    assert!(
        pump_until(&mut shell, |shell| shell.waiting()
            == Waiting::Question { options: 2 }),
        "the question should wait for an answer within {TIMEOUT:?}"
    );

    assert!(type_key(&mut shell, "2"), "a digit highlights an option");
    assert_eq!(shell.picked(), Some(1), "the second option is highlighted");
    assert_eq!(
        shell.waiting(),
        Waiting::Question { options: 2 },
        "highlighting is not answering"
    );

    assert!(press(&mut shell, Key::Enter), "Enter answers with the pick");
    assert_eq!(shell.picked(), None, "the pick is spent");
    assert!(
        pump_until(&mut shell, |shell| turns(shell) == 1),
        "the turn finishes once the question is answered"
    );
    let output = shell
        .transcript()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::Tool { output, .. } => Some(output.clone()),
            _ => None,
        })
        .expect("the question's call is a card");
    assert_eq!(output, "b", "the model is given the option the user picked");
}

#[test]
fn ctrl_c_stops_the_running_turn() {
    let (mut shell, _data, _worktree) = window_against("tests/fixtures/live_slow.txt");
    shell.set_prompt("go");
    press(&mut shell, Key::Enter);

    assert!(
        pump_until(&mut shell, |shell| !said(shell).is_empty()),
        "the reply should start streaming within {TIMEOUT:?}"
    );

    let interrupt = KeyEvent {
        key: Key::Text("c".to_string()),
        mods: Mods {
            ctrl: true,
            ..Mods::default()
        },
    };
    assert!(
        composer_key(&mut shell, &interrupt, &mut Tx::default()),
        "Ctrl+C is consumed by the window, not typed"
    );
    assert!(
        pump_until(&mut shell, |shell| turns(shell) >= 1),
        "a cancel still ends the turn"
    );
    assert!(
        said(&shell)
            .iter()
            .any(|text| text.to_lowercase().contains("cancelled")),
        "the column says the turn was cancelled: {:?}",
        said(&shell)
    );
}

#[test]
fn a_session_whose_worktree_vanished_refuses_the_prompt_with_a_reason() {
    let server = support::serve("tests/fixtures/live_text.txt");
    let data = tempfile::tempdir().unwrap();
    let worktree = tempfile::tempdir().unwrap();
    let path = worktree.path().to_path_buf();
    let mut app = AppState::open_with(data.path(), defaults(&server.base_url)).unwrap();
    let index = app.new_session(&path).unwrap();
    app.switch(index).unwrap();
    // The session's worktree is gone: the turn it would run has no root.
    drop(worktree);
    let mut shell = Shell::new(app.metas(), 0).with_live(app);

    shell.set_prompt("hello");
    assert!(
        press(&mut shell, Key::Enter),
        "the key is still the send key"
    );
    assert_eq!(
        shell.prompt_text(),
        "hello",
        "the text stays in the box, because there is nothing to retype"
    );
    assert!(
        asked(&shell).is_empty(),
        "a refused prompt adds no user turn: {:?}",
        asked(&shell)
    );
    let reason = said(&shell).join("\n");
    assert!(
        reason.contains("no longer exists"),
        "the column says why nothing ran: {reason:?}"
    );
    assert!(
        reason.contains(&path.display().to_string()),
        "the reason names the worktree: {reason:?}"
    );
    assert_eq!(turns(&shell), 0, "no turn ran");
}

#[test]
fn a_focused_composer_breaks_the_line_on_shift_enter() {
    let mut runtime = Runtime::new(
        Shell::idle(Vec::new(), 0),
        perch_ui::shell::view,
        (1280.0, 820.0),
        Env::new(true, 1.0),
    );
    runtime.frame(0.0);
    let field = runtime
        .elements()
        .into_iter()
        .find(|element| element.role == "textbox")
        .expect("the composer's field is in the tree");
    let at = (
        field.rect[0] + field.rect[2] * 0.5,
        field.rect[1] + field.rect[3] * 0.5,
    );
    runtime.pointer_down(at.0, at.1);
    runtime.pointer_up(at.0, at.1);
    runtime.frame(0.0);

    runtime.key(KeyEvent {
        key: Key::Text("a".to_string()),
        mods: Mods::default(),
    });
    runtime.frame(0.0);
    assert_eq!(runtime.state.prompt_text(), "a", "typing reaches the field");

    runtime.key(KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    });
    runtime.frame(0.0);
    assert_eq!(
        runtime.state.prompt_text(),
        "a",
        "Enter is the send key, never a character"
    );

    runtime.key(KeyEvent {
        key: Key::Enter,
        mods: Mods {
            shift: true,
            ..Mods::default()
        },
    });
    runtime.frame(0.0);
    assert_eq!(
        runtime.state.prompt_text(),
        "a\n",
        "Shift+Enter is the field's own line break"
    );
}

#[test]
fn a_failed_mcp_server_is_a_note_not_a_closed_window() {
    let data = tempfile::tempdir().unwrap();
    let worktree = tempfile::tempdir().unwrap();
    Settings {
        mcp_servers: vec![McpServerConfig::stdio(
            "broken",
            "/nonexistent/perch-mcp-fixture",
            Vec::new(),
        )],
        ..Settings::default()
    }
    .save(data.path())
    .unwrap();

    let mut app = AppState::open_with(data.path(), defaults("http://127.0.0.1:9"))
        .expect("a server that cannot load does not stop the app");
    let index = app.new_session(worktree.path()).unwrap();
    app.switch(index).unwrap();

    assert_eq!(app.metas().len(), 1, "the session opened anyway");
    let said: Vec<String> = app
        .active_session()
        .unwrap()
        .transcript()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::Assistant { text } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(
        said.iter().any(|text| text.contains("mcp broken failed")),
        "the server that failed is said in words: {said:?}"
    );
}
