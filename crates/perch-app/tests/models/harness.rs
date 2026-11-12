//! The app over a model-list table, and the readers these tests use on it.

use std::sync::{Arc, Mutex};

use perch_agent::{Error as AgentError, ModelTransport};
use perch_app::{AppState, ProviderDefaults};
use perch_store::SessionMeta;
use perch_store::secret::MemorySecrets;
use perch_store::session::meta_path;
use perch_ui::Item;
use tempfile::TempDir;

/// What the table answers.
pub enum Answer {
    /// A 200 with this body.
    Body(&'static str),
    /// A refusal with this status.
    Status(u16),
}

/// A transport that records every URL and answers the one thing it was given.
pub struct Table {
    answer: Answer,
    pub asked: Mutex<Vec<String>>,
}

impl Table {
    pub fn new(answer: Answer) -> Arc<Table> {
        Arc::new(Table {
            answer,
            asked: Mutex::new(Vec::new()),
        })
    }

    /// The URL of the request at `index`.
    pub fn url(&self, index: usize) -> String {
        let asked = self.asked.lock().unwrap();
        asked
            .get(index)
            .unwrap_or_else(|| panic!("no request number {index}: {asked:?}"))
            .clone()
    }
}

impl ModelTransport for Table {
    fn get(&self, url: &str, _api_key: &str) -> perch_agent::Result<Vec<u8>> {
        self.asked.lock().unwrap().push(url.to_string());
        match self.answer {
            Answer::Body(body) => Ok(body.as_bytes().to_vec()),
            Answer::Status(status) => Err(AgentError::Status {
                status,
                body: "no such endpoint".to_string(),
            }),
        }
    }
}

/// The defaults a test starts from: one provider, and the model it names.
pub fn defaults(base_url: &str, model: &str) -> ProviderDefaults {
    ProviderDefaults {
        base_url: base_url.to_string(),
        api_key: "sk-test".to_string(),
        model: model.to_string(),
    }
}

/// An app over `data` that never reaches the keychain.
pub fn app(data: &TempDir, defaults: ProviderDefaults) -> AppState {
    hermetic();
    AppState::open_with_secrets(data.path(), defaults, Box::new(MemorySecrets::new()))
        .expect("a data directory with no sessions opens")
}

/// The words the active session's transcript says, joined.
pub fn said(app: &AppState) -> String {
    app.active_session()
        .expect("a session is open")
        .transcript()
        .items()
        .iter()
        .filter_map(|item| match item {
            // Both voices count here: this helper answers "what does the column say", and a
            // refusal or a notice is something the column says, even though it is not a reply.
            Item::Assistant { text } => Some(text.as_str()),
            Item::Note(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// The model a session's log header names, read from disk.
pub fn header_model(data: &TempDir, meta: &SessionMeta) -> String {
    let text = std::fs::read_to_string(meta_path(data.path(), &meta.id)).unwrap();
    let first = text.lines().next().expect("the log has a header");
    serde_json::from_str::<SessionMeta>(first)
        .expect("the header parses")
        .model
}

/// Tests must not read the machine they run on: a real `AGENTS.md` would join every system prompt
/// and change what the column says. `PERCH_HOME` is the seam the instruction loader documents.
fn hermetic() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let home = std::env::temp_dir().join("perch-tests-home");
        let _ = std::fs::create_dir_all(&home);
        // Safety: set once per process, before any app is built, and only under test.
        unsafe { std::env::set_var("PERCH_HOME", &home) };
    });
}
