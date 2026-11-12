//! The update check URL as the settings panel saves it: written to `settings.toml`, and the check
//! it names started or stopped with no restart.
//!
//! The transport is a table that refuses every route, so the check runs and fails in words without
//! opening a socket — the point here is that the thread was started at all, not what the server
//! said.

use std::sync::Arc;
use std::time::{Duration, Instant};

use perch_app::update::OWNER;
use perch_app::{AppState, ProviderDefaults};
use perch_store::Settings;
use perch_store::secret::MemorySecrets;
use perch_ui::SettingsDraft;
use perch_ui::panel_lines;
use perch_ui::shell::Panels;
use perch_update::Transport;

/// The manifest a test names in `settings.toml`.
const MANIFEST: &str = "https://release.perch.test/latest.json";
/// How long a test waits for the update thread to answer. Bounded, not timed.
const TIMEOUT: Duration = Duration::from_secs(20);

/// A transport with no route at all: every check fails immediately, and nothing is fetched.
struct Unrouted;

impl Transport for Unrouted {
    fn get(&self, url: &str) -> perch_update::Result<Vec<u8>> {
        Err(perch_update::Error::Transport {
            reason: format!("no route for {url}"),
        })
    }
}

/// A draft with every field stated, so the test names only the URL it changes.
fn draft(update_check: &str) -> SettingsDraft {
    SettingsDraft {
        base_url: "https://example.com/v1".to_string(),
        model: "test/model".to_string(),
        theme: "dark".to_string(),
        font_size: String::new(),
        update_check: update_check.to_string(),
        api_key: String::new(),
        profile: String::new(),
        activate: false,
        keybinds: Default::default(),
    }
}

/// The words the update panel says, when it is drawn.
fn update_words(app: &AppState) -> Option<String> {
    Panels::panels(app)
        .into_iter()
        .find(|entry| entry.plugin == OWNER)
        .map(|entry| panel_lines(&entry.model).join(" "))
}

/// Pumps frames until `done` holds, or [`TIMEOUT`] runs out.
fn pump_until(app: &mut AppState, done: impl Fn(&AppState) -> bool) -> bool {
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        if done(app) {
            return true;
        }
        app.pump(Duration::from_millis(5));
    }
    done(app)
}

#[test]
fn a_saved_update_url_is_checked_without_a_restart_and_clearing_it_stops_the_check() {
    let data = tempfile::tempdir().unwrap();
    let mut app = AppState::open_with_update(
        data.path(),
        ProviderDefaults::default(),
        Box::new(MemorySecrets::default()),
        Arc::new(Unrouted),
    )
    .expect("the app opens");

    // The empty default is no check at all: no thread, no panel, no request.
    assert!(
        update_words(&app).is_none(),
        "a window with no manifest URL drew an update panel"
    );

    app.save_settings(draft(MANIFEST)).expect("the save wrote");

    assert_eq!(
        Settings::load(data.path()).unwrap().update_check,
        MANIFEST,
        "the URL is saved with the other settings"
    );
    assert_eq!(
        app.settings().update_check,
        MANIFEST,
        "the panel reads back what it saved"
    );

    // The check ran on the thread the save started, and its failure is the line the column says.
    assert!(
        pump_until(&mut app, |app| update_words(app)
            .is_some_and(|words| words.contains("update check failed"))),
        "the saved URL did not start a check without a restart: {:?}",
        update_words(&app)
    );

    // Clearing the field is the other half of the same promise: the thread is gone, so nothing is
    // checked and the column stops drawing the offer.
    app.save_settings(draft("")).expect("the clear wrote");
    assert_eq!(
        Settings::load(data.path()).unwrap().update_check,
        "",
        "an empty URL is saved, not left behind"
    );
    assert!(
        update_words(&app).is_none(),
        "clearing the manifest URL left the check running: {:?}",
        update_words(&app)
    );
}
