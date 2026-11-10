//! A session's own events reaching a plugin.
//!
//! The app has no event tap, but it appends every event to the session's log and flushes each line,
//! so the log is what the window feeds the plugins from. This test writes that log the way a turn
//! does and reads the panel the plugin answers with.

mod support;

use perch_app::Sources;
use perch_store::SessionEvent;
use support::{app, dir_with, session};

#[test]
fn a_sessions_logged_events_reach_the_plugin_and_change_its_panel() {
    let data = tempfile::tempdir().unwrap();
    let mut log = session(data.path());
    log.append(&SessionEvent::TurnEnd {
        reason: "done".to_string(),
        tokens_in: 1,
        tokens_out: 2,
    })
    .unwrap();
    let app = app(data.path());

    let plugins = dir_with(&[("counter.wasm", "counter.wasm")]);
    let mut sources = Sources::open(plugins.path().to_path_buf(), &[]).unwrap();
    let log = &app.active_session().expect("the app opened a session").log;
    assert!(
        sources.sync(log),
        "the turn end already in the log was never delivered"
    );

    let panel = sources
        .panels()
        .into_iter()
        .find(|entry| entry.plugin == "counter")
        .expect("the counter panel");
    assert_eq!(
        perch_ui::panel_lines(&panel.model),
        vec!["count: 1".to_string()],
        "the plugin did not see the session's turn end"
    );

    // Once only: the feed reads the bytes it has not read, so a second pass adds nothing.
    assert!(!sources.sync(log), "the same event was delivered twice");
    let panel = sources
        .panels()
        .into_iter()
        .next()
        .expect("the counter panel");
    assert_eq!(
        perch_ui::panel_lines(&panel.model),
        vec!["count: 1".to_string()]
    );
}
