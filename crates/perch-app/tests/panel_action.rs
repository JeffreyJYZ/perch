//! A panel action through the app: the worker owns the plugin host, so the press is queued and the
//! snapshot that follows is where the change is read.
//!
//! These are the two halves of the seam: a press that is accepted changes a panel the column
//! draws, and a press that cannot be addressed is words instead of work queued for a guest that
//! would ignore it. Nothing here needs a provider: no turn runs.

mod support;

use std::time::{Duration, Instant};

use perch_app::AppState;
use perch_plugins::PanelSection;
use perch_ui::shell::Panels;
use support::{app, session};

/// A data directory with a plugin directory holding `files`, and a session over it.
fn data_with(files: &[(&str, &str)]) -> tempfile::TempDir {
    let data = tempfile::tempdir().unwrap();
    let plugins = data.path().join(perch_app::settings::PLUGINS);
    std::fs::create_dir_all(&plugins).unwrap();
    for (from, to) in files {
        std::fs::copy(support::fixtures().join(from), plugins.join(to)).unwrap();
    }
    session(data.path());
    data
}

/// The app, once the worker has published the tools and panels it loads.
///
/// The worker builds off the frame thread and hands back the built-ins first, so an app straight
/// out of `open` has no panel in its snapshot yet — the window draws that empty column for one
/// frame, which is the point. A test that wants to press a control has to be past it.
fn opened(dir: &std::path::Path) -> AppState {
    let mut state = app(dir);
    settled(&mut state);
    state
}

/// Pumps until the worker has published the tools, plugins and panels it loads.
///
/// `busy()` is the worker's own flag: it is set before the thread starts loading and cleared only
/// after it has published, so waiting on it waits exactly as long as a cold wasmtime compile takes
/// and no longer. A fixed iteration count is not enough — the first fixture load on a busy box
/// outlives it, and then the snapshot read is the built-ins and the test blames the code.
fn settled(state: &mut AppState) {
    let deadline = Instant::now() + Duration::from_secs(120);
    while state.busy() && Instant::now() < deadline {
        state.pump(Duration::from_millis(10));
    }
    // The last snapshot is read on a pump, not on the worker's publish.
    state.pump(Duration::from_millis(10));
    assert!(
        !state.busy(),
        "the worker never finished loading the plugins"
    );
}

/// Every text line of `plugin`'s panel as the column would draw it.
fn lines(state: &AppState, plugin: &str) -> Vec<String> {
    Panels::panels(state)
        .into_iter()
        .find(|entry| entry.plugin == plugin)
        .map(|entry| {
            entry
                .model
                .sections
                .into_iter()
                .filter_map(|section| match section {
                    PanelSection::Text(text) => Some(text),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_button_press_reaches_the_plugin_and_the_column_changes() {
    let data = data_with(&[("counter.wasm", "counter.wasm")]);
    let mut state = opened(data.path());
    // The window opens before the sources are built, so the first snapshot carries the panels:
    // a test that read them on the open would be reading the frame before the plugins loaded.
    settled(&mut state);
    assert_eq!(
        lines(&state, "counter"),
        ["count: 0"],
        "{:?}",
        Panels::plugin_words(&state)
    );

    // A button press carries no value; the app accepts it and the worker answers it.
    state
        .panel_action("counter", "bump", None)
        .expect("the button press was refused");
    settled(&mut state);
    assert_eq!(
        lines(&state, "counter"),
        ["count: 1"],
        "the press did not come back as a changed panel"
    );

    // An input submit carries its text, and the same path answers it.
    state
        .panel_action("counter", "note", Some("hello".to_string()))
        .expect("the input submit was refused");
    settled(&mut state);
    assert_eq!(lines(&state, "counter"), ["count: 1", "note: hello"]);
}

#[test]
fn a_press_for_a_control_the_panel_does_not_declare_is_stated() {
    let data = data_with(&[("counter.wasm", "counter.wasm")]);
    let mut state = opened(data.path());
    // The window opens before the sources are built, so the first snapshot carries the panels:
    // a test that read them on the open would be reading the frame before the plugins loaded.
    settled(&mut state);

    let refused = state
        .panel_action("counter", "no-such-control", None)
        .expect_err("an undeclared control was accepted");
    assert!(
        refused.contains("counter") && refused.contains("declares no control `no-such-control`"),
        "the refusal does not name what was pressed: {refused}"
    );
    settled(&mut state);
    assert_eq!(
        lines(&state, "counter"),
        ["count: 0"],
        "a refused press still reached the plugin"
    );
}

#[test]
fn a_press_for_a_plugin_that_is_gone_is_stated() {
    let data = data_with(&[("counter.wasm", "counter.wasm")]);
    let state = opened(data.path());

    let refused = state
        .panel_action("no-such-plugin", "bump", None)
        .expect_err("a press for a plugin with no panel was accepted");
    assert!(
        refused.contains("no-such-plugin") && refused.contains("not drawing a panel"),
        "the refusal does not say what is gone: {refused}"
    );
}

#[test]
fn a_press_that_traps_disables_the_plugin_and_says_so_in_the_column() {
    let data = data_with(&[("trap-action/trap-action.wasm", "trap-action.wasm")]);
    let mut state = opened(data.path());
    // The window opens before the sources are built, so the first snapshot carries the panels:
    // a test that read them on the open would be reading the frame before the plugins loaded.
    settled(&mut state);
    assert_eq!(lines(&state, "trap-action"), ["press the button"]);

    // The press is accepted; the trap happens on the worker's thread, which is where the host is.
    state
        .panel_action("trap-action", "trap", None)
        .expect("the press was refused before it ran");
    settled(&mut state);

    assert!(
        Panels::panels(&state).is_empty(),
        "a plugin that trapped is still drawing a panel"
    );
    let words = Panels::plugin_words(&state);
    assert!(
        words
            .iter()
            .any(|word| word.contains("trap-action") && word.contains("disabled")),
        "nothing says which plugin went: {words:?}"
    );

    // A second press has nowhere to go, and says so rather than failing silently.
    let refused = state
        .panel_action("trap-action", "trap", None)
        .expect_err("a press reached a plugin that trapped");
    assert!(
        refused.contains("not drawing a panel"),
        "the refusal does not say the panel is gone: {refused}"
    );
}
