//! The strip's order and its one active tab, and Escape as the way back to the transcript.

use perch_ui::Shell;
use perch_ui::shell::Pane;
use rune::{Key, KeyEvent, Mods, Runtime};

use super::{open, over, tab_named, tabs_in};
use crate::harness::click;

/// How many elements the last frame drew under `key`.
fn drawn(runtime: &Runtime<Shell>, key: &str) -> usize {
    runtime
        .elements()
        .into_iter()
        .filter(|element| element.key.as_deref() == Some(key))
        .count()
}

/// A bare Escape, which is the way back to the transcript.
fn escape(runtime: &mut Runtime<Shell>) {
    runtime.key(KeyEvent {
        key: Key::Escape,
        mods: Mods::default(),
    });
    runtime.frame(1.0 / 60.0);
}

#[test]
fn the_strip_is_the_sessions_then_the_files_then_the_settings() {
    let (_repo, mut runtime) = over(&[("a.rs", "fn a() {}\n")]);
    open(&mut runtime, "a.rs");

    let tree = runtime.accessibility_json();
    let session = tab_named(&tree, "work").expect("the session's tab");
    let file = tab_named(&tree, "a.rs").expect("the file's tab");
    let settings = tab_named(&tree, "Settings").expect("the settings tab");
    assert_eq!(
        (
            session["position"].as_u64(),
            file["position"].as_u64(),
            settings["position"].as_u64()
        ),
        (Some(1), Some(2), Some(3)),
        "session, then the files in the order they were opened, then the settings"
    );
}

#[test]
fn each_pane_is_drawn_once_with_one_active_tab() {
    let (_repo, mut runtime) = over(&[("a.rs", "fn a() {}\n")]);

    // The session: the transcript column, and its tab alone.
    assert_eq!(drawn(&runtime, "transcript-column"), 1);
    assert_eq!(drawn(&runtime, "preview"), 0);
    assert_eq!(tabs_in(&runtime, "selected"), vec!["t-work".to_string()]);

    // A file: the preview, and no transcript column beside it.
    open(&mut runtime, "a.rs");
    assert_eq!(drawn(&runtime, "preview"), 1);
    assert_eq!(drawn(&runtime, "transcript-column"), 0);
    assert_eq!(tabs_in(&runtime, "selected").len(), 1);

    // The settings: the form, once.
    click(&mut runtime, "t-settings");
    assert_eq!(drawn(&runtime, "settings"), 1);
    assert_eq!(drawn(&runtime, "preview"), 0);
    assert_eq!(drawn(&runtime, "transcript-column"), 0);
    assert_eq!(
        tabs_in(&runtime, "selected"),
        vec!["t-settings".to_string()]
    );
}

#[test]
fn the_session_tab_already_showing_returns_to_the_transcript_and_keeps_the_file_tabs() {
    let (_repo, mut runtime) = over(&[("a.rs", "fn a() {}\n")]);
    open(&mut runtime, "a.rs");
    click(&mut runtime, "t-work");

    assert_eq!(runtime.state.pane(), &Pane::Session);
    assert_eq!(
        runtime.state.file_tabs().len(),
        1,
        "the session already showing is not a switch, so its file tabs stay"
    );
}

#[test]
fn escape_leaves_the_file_pane_and_the_settings_for_the_transcript() {
    let (_repo, mut runtime) = over(&[("a.rs", "fn a() {}\n")]);
    open(&mut runtime, "a.rs");
    escape(&mut runtime);
    assert_eq!(runtime.state.pane(), &Pane::Session);
    assert_eq!(
        runtime.state.file_tabs().len(),
        1,
        "the tab stays in the strip: Escape is a way back, not a close"
    );
    assert_eq!(tabs_in(&runtime, "selected"), vec!["t-work".to_string()]);

    click(&mut runtime, "t-settings");
    assert!(runtime.state.settings_open());
    escape(&mut runtime);
    assert!(!runtime.state.settings_open(), "Escape leaves the settings");
    assert_eq!(tabs_in(&runtime, "selected"), vec!["t-work".to_string()]);
}
