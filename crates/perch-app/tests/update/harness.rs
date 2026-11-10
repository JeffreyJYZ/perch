//! The fake release server the update tests talk to, and the app opened over it.
//!
//! Nothing here opens a socket: the transport is a table of routes plus a record of what was
//! asked for, so a test proves both the answer and that nothing else was fetched.

#![allow(dead_code)]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use perch_app::update::OWNER;
use perch_app::{AppState, ProviderDefaults};
use perch_plugins::PanelEntry;
use perch_store::Settings;
use perch_store::secret::MemorySecrets;
use perch_ui::plugin_panel::panel_lines;
use perch_ui::shell::Panels;
use perch_update::{Error, Result, Transport};
use tempfile::TempDir;

/// The manifest URL the tests write into `settings.toml`.
pub const MANIFEST: &str = "https://release.perch.test/latest.json";
/// The archive URL every test manifest names.
pub const ARCHIVE: &str = "https://release.perch.test/Perch.zip";
/// SHA-256 of the bytes "hello", computed away from this crate.
pub const HELLO: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
/// How long a test waits for the worker to publish. Bounded, not timed.
pub const TIMEOUT: Duration = Duration::from_secs(20);

/// A transport with routes, an optional delay, and a record of every URL it was asked for.
pub struct Fake {
    routes: Vec<(String, Vec<u8>)>,
    delay: Duration,
    asked: Mutex<Vec<String>>,
    threads: Mutex<Vec<std::thread::ThreadId>>,
}

impl Fake {
    /// A server that answers at once.
    pub fn new(routes: &[(&str, &str)]) -> Arc<Fake> {
        Fake::slow(routes, Duration::ZERO)
    }

    /// A server that takes `delay` to answer every request, for proving frames keep coming.
    pub fn slow(routes: &[(&str, &str)], delay: Duration) -> Arc<Fake> {
        Arc::new(Fake {
            routes: routes
                .iter()
                .map(|(route, body)| ((*route).to_string(), body.as_bytes().to_vec()))
                .collect(),
            delay,
            asked: Mutex::new(Vec::new()),
            threads: Mutex::new(Vec::new()),
        })
    }

    /// Every URL asked for, in order.
    pub fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }

    /// The thread every request was made on, which is how a test proves it was not the frame's.
    pub fn threads(&self) -> Vec<std::thread::ThreadId> {
        self.threads.lock().unwrap().clone()
    }
}

impl Transport for Fake {
    fn get(&self, url: &str) -> Result<Vec<u8>> {
        self.asked.lock().unwrap().push(url.to_string());
        self.threads
            .lock()
            .unwrap()
            .push(std::thread::current().id());
        if !self.delay.is_zero() {
            std::thread::sleep(self.delay);
        }
        self.routes
            .iter()
            .find(|(route, _)| route == url)
            .map(|(_, body)| body.clone())
            .ok_or_else(|| Error::Transport {
                reason: format!("no route for {url}"),
            })
    }
}

/// One release document, with `notes` this test can assert verbatim.
pub fn manifest(version: &str, sha256: &str) -> String {
    serde_json::json!({
        "version": version,
        "url": ARCHIVE,
        "notes": "Fixed the updater.",
        "min_os": 1,
        "sha256": sha256,
    })
    .to_string()
}

/// A data directory whose settings name `url`, or name none when it is `None`.
pub fn data(url: Option<&str>) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    Settings {
        update_check: url.unwrap_or_default().to_string(),
        ..Settings::default()
    }
    .save(dir.path())
    .unwrap();
    dir
}

/// The provider a test names, which nothing here ever reaches.
pub fn defaults() -> ProviderDefaults {
    ProviderDefaults {
        base_url: "http://127.0.0.1:1/v1".to_string(),
        api_key: String::new(),
        model: "test/model".to_string(),
    }
}

/// An app over `data` whose update traffic goes to `fake` and never to a socket.
pub fn app(data: &TempDir, fake: Arc<Fake>) -> AppState {
    let transport: Arc<dyn Transport> = fake;
    AppState::open_with_update(
        data.path(),
        defaults(),
        Box::new(MemorySecrets::default()),
        transport,
    )
    .expect("the app opens over the data directory")
}

/// The update entry the column draws, when there is one.
pub fn entry(app: &AppState) -> Option<PanelEntry> {
    Panels::panels(app)
        .into_iter()
        .find(|entry| entry.plugin == OWNER)
}

/// Every line the update panel says.
pub fn lines(app: &AppState) -> Vec<String> {
    entry(app).map_or_else(Vec::new, |entry| panel_lines(&entry.model))
}

/// The first, and normally only, line the update panel says.
pub fn line(app: &AppState) -> Option<String> {
    lines(app).into_iter().next()
}

/// Pumps frames until `done` holds, or [`TIMEOUT`] runs out.
pub fn pump_until(app: &mut AppState, done: impl Fn(&AppState) -> bool) -> bool {
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        if done(app) {
            return true;
        }
        app.pump(Duration::from_millis(5));
    }
    done(app)
}
