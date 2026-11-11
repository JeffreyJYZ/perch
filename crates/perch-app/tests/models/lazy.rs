//! The model list is fetched off the click path, and its limits reach the meter.
//!
//! The transport here blocks until the test lets it go, which is what makes "returns at once" a
//! measurement rather than a hope: a window that waited would sit for five seconds.

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use perch_agent::{ModelTransport, Result as AgentResult};
use perch_app::AppState;

use crate::harness::{app, defaults};

/// The body the gate lists once it is released: one model that states a window, one that does not.
const BODY: &str = r#"{"data":[
    {"id":"vendor/wide","context_length":200000},
    {"id":"vendor/narrow"},
    {"id":"vendor/silent"}
]}"#;

/// A transport that holds every request until the test sends it through.
pub struct Gate {
    release: Mutex<Receiver<()>>,
    asked: Mutex<Vec<String>>,
}

impl Gate {
    /// A gate and the one-shot sender that opens it.
    pub fn closed() -> (Arc<Gate>, Sender<()>) {
        let (tx, rx) = std::sync::mpsc::channel();
        let gate = Arc::new(Gate {
            release: Mutex::new(rx),
            asked: Mutex::new(Vec::new()),
        });
        (gate, tx)
    }

    /// The URLs asked for so far.
    pub fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }
}

impl ModelTransport for Gate {
    fn get(&self, url: &str, _api_key: &str) -> AgentResult<Vec<u8>> {
        self.asked.lock().unwrap().push(url.to_string());
        let release = self.release.lock().unwrap();
        let _ = release.recv_timeout(Duration::from_secs(5));
        Ok(BODY.as_bytes().to_vec())
    }
}

/// An app with one session whose model is the one the gate lists.
pub fn gated(
    data: &tempfile::TempDir,
    worktree: &tempfile::TempDir,
) -> (AppState, Arc<Gate>, Sender<()>) {
    let (gate, open) = Gate::closed();
    // Clone through the concrete `Arc<Gate>` first: `Arc::clone` with only the unsized target
    // type in scope asks the argument to unsize, and `&Arc<Gate>` cannot.
    let listings: Arc<dyn ModelTransport> = gate.clone();
    let mut app =
        app(data, defaults("https://provider.test/v1", "vendor/wide")).with_listings(listings);
    let index = app.new_session(worktree.path()).unwrap();
    app.switch(index).unwrap();
    (app, gate, open)
}

#[test]
fn starting_a_listing_returns_at_once_and_the_answer_carries_the_stated_window() {
    let data = tempfile::tempdir().unwrap();
    let worktree = tempfile::tempdir().unwrap();
    let (mut app, gate, open) = gated(&data, &worktree);

    let started = Instant::now();
    app.start_models();
    assert!(
        started.elapsed() < Duration::from_millis(250),
        "a blocked provider must not hold the caller: {:?}",
        started.elapsed()
    );
    assert!(
        app.models_ready().is_none(),
        "the answer cannot exist before the transport is released"
    );

    // A second press while the first is out is not a second request.
    app.start_models();
    assert_eq!(gate.asked().len(), 1, "one listing on its way at a time");

    open.send(()).unwrap();
    let listed = wait_for_models(&mut app).expect("the gate answers once released");
    assert_eq!(
        listed
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>(),
        ["vendor/narrow", "vendor/silent", "vendor/wide"]
    );
    assert_eq!(
        app.model_limit(),
        Some(200_000),
        "the chosen model's window is reachable from the app"
    );
}

/// Polls `models_ready` until it answers, with a deadline so a stuck job fails the test.
pub fn wait_for_models(app: &mut AppState) -> Option<Vec<perch_agent::ListedModel>> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        match app.models_ready() {
            Some(Ok(models)) => return Some(models),
            Some(Err(words)) => panic!("the listing failed: {words}"),
            None => std::thread::sleep(Duration::from_millis(2)),
        }
    }
    None
}
