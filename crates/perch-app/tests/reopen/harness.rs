//! An app opened over a written log, and the readers a test uses on it.

use std::io::Write;
use std::path::Path;
use std::sync::{Once, OnceLock};
use std::time::Duration;

use perch_app::{AppState, ProviderDefaults};
use perch_store::{SessionEvent, SessionLog, SessionMeta, meta_path};
use perch_ui::Item;
use tempfile::TempDir;

/// How long a test waits for a turn to end.
pub const TIMEOUT: Duration = Duration::from_secs(10);

/// The one session a test writes.
const ID: &str = "reopened";

/// The two directories a reopened app needs: the data directory that holds the log, and the
/// worktree that log names. Both outlive the app, which reads real paths.
pub fn place() -> (TempDir, TempDir) {
    (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap())
}

/// Writes the session a test then reopens, holding `events` in order.
///
/// The title is a name of its own rather than the folder's, so the titler leaves the session
/// alone: a title request would land on the fixture's socket and the count of what a turn asked
/// would stop meaning one request.
pub fn log(dir: &Path, worktree: &Path, events: &[SessionEvent]) {
    let mut log = SessionLog::open(
        dir,
        SessionMeta {
            id: ID.to_string(),
            title: "earlier work".to_string(),
            worktree: worktree.to_path_buf(),
            branch: String::new(),
            model: "test/model".to_string(),
            created: 0,
        },
    )
    .expect("the log opens");
    for event in events {
        log.append(event).expect("the log appends");
    }
}

/// Appends `bytes` to the log as they are, which is how a torn or corrupt line is written.
pub fn damage(dir: &Path, bytes: &str) {
    std::fs::OpenOptions::new()
        .append(true)
        .open(meta_path(dir, ID))
        .expect("the log opens for damage")
        .write_all(bytes.as_bytes())
        .expect("the damage is written");
}

/// One exchange as the log writes it: the question, the reply, and the turn ending on its own.
pub fn exchange(question: &str, answer: &str) -> Vec<SessionEvent> {
    vec![
        SessionEvent::User {
            text: question.to_string(),
        },
        SessionEvent::Assistant {
            text: answer.to_string(),
        },
        SessionEvent::TurnEnd {
            reason: "done".to_string(),
            tokens_in: 5,
            tokens_out: 2,
        },
    ]
}

/// An app opened over the log in `dir`, talking to `fixture`.
pub fn open(dir: &Path, fixture: &str) -> (AppState, crate::support::Fixture) {
    quiet_home();
    let server = crate::support::serve(fixture);
    // No machine-wide instructions: see `crate::support::hermetic`.
    crate::support::hermetic();
    let app = AppState::open_with(
        dir,
        ProviderDefaults {
            base_url: server.base_url.clone(),
            api_key: "test-key".to_string(),
            model: "test/model".to_string(),
        },
    )
    .expect("the app opens over the log");
    (app, server)
}

/// Points the person's own instruction files at an empty home.
///
/// Every session's system message carries the global `AGENTS.md` files, read from `PERCH_HOME` or
/// the real home, and a large one is said in the column as a note. A test about what a log replays
/// has to be told about none of them, or the machine's rules would be the last row of every column
/// compared here. Set once, so no two tests race the environment.
fn quiet_home() {
    static HOME: OnceLock<TempDir> = OnceLock::new();
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let home = HOME.get_or_init(|| tempfile::tempdir().expect("a home directory"));
        // The seam perch-agent documents for a test or a sandbox.
        unsafe { std::env::set_var("PERCH_HOME", home.path()) };
    });
}

/// The active session's column, which is the one session a test writes.
fn items(app: &AppState) -> &[Item] {
    app.active_session()
        .expect("the log opened a session")
        .transcript()
        .items()
}

/// The column as one label per item, so a test can read the order it was replayed in.
pub fn shape(app: &AppState) -> Vec<String> {
    items(app)
        .iter()
        .map(|item| match item {
            Item::User { text } => format!("user: {text}"),
            Item::Assistant { text } => format!("assistant: {text}"),
            Item::Tool {
                name,
                status,
                output,
                ..
            } => format!("tool: {name} {status} {output}"),
            Item::Note(text) => format!("note: {text}"),
            Item::Reasoning { .. } => "reasoning".to_string(),
            Item::Summary { tools, tokens, .. } => match tokens {
                Some(tokens) => format!("summary: {tools} tools, {tokens} tokens"),
                None => format!("summary: {tools} tools, no tokens"),
            },
        })
        .collect()
}

/// Every note the column says, in order.
pub fn notes(app: &AppState) -> Vec<String> {
    items(app)
        .iter()
        .filter_map(|item| match item {
            Item::Note(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// What the person asked, in order, as the column replays it.
pub fn users(app: &AppState) -> Vec<String> {
    items(app)
        .iter()
        .filter_map(|item| match item {
            Item::User { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// The body of the first request the fixture received.
pub fn first_request(fixture: &crate::support::Fixture) -> String {
    let bodies = fixture.bodies.lock().unwrap();
    bodies
        .first()
        .cloned()
        .expect("the fixture never received a request")
}
