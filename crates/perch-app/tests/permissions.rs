//! The saved permission rules, end to end: a saved allow dispatches a gated call without asking,
//! and a file that will not parse leaves the app with no rules at all and one line saying why.

#[path = "../../perch-agent/tests/support/mod.rs"]
mod support;

use std::path::Path;
use std::time::{Duration, Instant};

use perch_app::{AppState, ProviderDefaults};
use perch_store::settings::rules_path;
use perch_ui::Item;
use tempfile::TempDir;

/// A recorded turn that writes `made.txt` in the worktree, which is a gated call.
const FIXTURE: &str = "tests/fixtures/live_write.txt";
/// How long the turn is given to settle. Bounded, not timed.
const TIMEOUT: Duration = Duration::from_secs(10);

fn defaults(base_url: &str) -> ProviderDefaults {
    ProviderDefaults {
        base_url: base_url.to_string(),
        api_key: "test-key".to_string(),
        model: "test/model".to_string(),
    }
}

/// An app over `data` whose session works in `worktree`, talking to the write fixture.
fn app(data: &Path, worktree: &Path) -> (AppState, support::Fixture) {
    let server = support::serve(FIXTURE);
    // No machine-wide instructions: this test is about permissions, not about the home directory.
    support::hermetic();
    let mut app = AppState::open_with(data, defaults(&server.base_url)).expect("the app opens");
    let index = app.new_session(worktree).expect("a session is added");
    app.switch(index).expect("the session is shown");
    (app, server)
}

/// Writes one line of rules, or one line that is not a rule at all.
fn save(data: &Path, line: &str) {
    std::fs::write(rules_path(data), format!("{line}\n")).expect("the rules file is written");
}

/// Runs the fixture's turn, answering nothing, and answers whether it stopped to ask approval.
fn asked(app: &mut AppState) -> bool {
    app.prompt("write it").expect("the prompt is taken");
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        app.pump(Duration::from_millis(20));
        let Some(session) = app.active_session() else {
            panic!("the session went away");
        };
        if session.is_waiting() {
            return true;
        }
        if !session.is_running() {
            return false;
        }
    }
    panic!("the turn never settled within {TIMEOUT:?}");
}

/// Everything the column says, as the user reads it.
fn said(app: &AppState) -> String {
    app.active_session()
        .map(|session| {
            session
                .transcript()
                .items()
                .iter()
                .filter_map(|item| match item {
                    Item::Assistant { text } => Some(text.as_str()),
                    Item::Note(text) => Some(text.as_str()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_saved_allow_dispatches_the_call_without_asking() {
    let data = TempDir::new().unwrap();
    let worktree = TempDir::new().unwrap();
    save(data.path(), r#"{"tool":"write","decision":"allow"}"#);

    let (mut app, _server) = app(data.path(), worktree.path());
    assert!(
        !asked(&mut app),
        "a saved allow was not applied: the call asked for approval"
    );
    assert!(
        worktree.path().join("made.txt").exists(),
        "the allowed write never ran: {}",
        said(&app)
    );
}

#[test]
fn a_file_that_will_not_parse_is_no_rules_and_one_line() {
    let data = TempDir::new().unwrap();
    let worktree = TempDir::new().unwrap();
    save(data.path(), "this is not a rule");

    let (mut app, _server) = app(data.path(), worktree.path());
    let said = said(&app);
    assert!(
        said.contains("saved permissions were not read"),
        "a corrupt file was not named: {said:?}"
    );
    assert!(
        asked(&mut app),
        "a corrupt rules file must not allow anything: the call went through without asking"
    );
}
