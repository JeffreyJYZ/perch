//! The strip's order and its one active tab, Escape as the way back, and the close press.

use perch_ui::Shell;
use perch_ui::shell::Pane;
use rune::{Key, KeyEvent, Mods, Runtime};

use super::{open, over, tab_named, tabs_in};
use crate::harness::click;

/// How many nodes the drawn tree holds under `key`, so a pane is counted where it is painted and
/// not in the runtime's retained scene, which keeps an outgoing pane until its exit finishes.
fn drawn(runtime: &Runtime<Shell>, key: &str) -> usize {
    let mut found = 0;
    let mut stack = vec![runtime.accessibility_json()];
    while let Some(node) = stack.pop() {
        if node["key"] == key {
            found += 1;
        }
        if let Some(children) = node["children"].as_array() {
            stack.extend(children.iter().cloned());
        }
    }
    found
}

/// The id of the one file tab the strip carries: the key the close press names.
fn file_tab_id(runtime: &Runtime<Shell>) -> String {
    runtime
        .state
        .file_tabs()
        .first()
        .map(|path| path.display().to_string())
        .expect("a file tab is open")
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

#[test]
fn closing_the_file_tab_returns_to_the_session_and_keeps_the_tree_selection() {
    let (_repo, mut runtime) = over(&[("a.rs", "fn a() {}\n")]);
    open(&mut runtime, "a.rs");
    // The strip's close control runs exactly this: `close_tab` with the id the tab is drawn
    // under. The control itself cannot be pressed headlessly: it closes on the press, the strip
    // reflows, and the release then lands on the tab that moved under the pointer.
    assert!(runtime.state.close_tab(&file_tab_id(&runtime)), "it closes");
    // A closed tab leaves on its exit animation, so the strip still lists it — and still marks it
    // selected — for the frames that animation runs. Settle before reading what is drawn.
    for _ in 0..40 {
        runtime.frame(1.0 / 60.0);
    }

    assert_eq!(runtime.state.pane(), &Pane::Session);
    assert!(runtime.state.file_tabs().is_empty(), "the tab is gone");
    assert!(
        runtime
            .state
            .files()
            .and_then(|panel| panel.selected_path())
            .is_some(),
        "the tree keeps its selection, so the file is one press away"
    );
    // The strip is drawn from state, and `close_tab` was called on the state directly, so the
    // frame has nothing to repaint until a handler runs. Pressing the session tab is the gesture
    // that dirties it, and the one a person would make next.
    click(&mut runtime, "t-work");
    assert_eq!(tabs_in(&runtime, "selected"), vec!["t-work".to_string()]);
}
