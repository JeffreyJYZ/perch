//! The settings pane over the app: opening it never waits for the provider, and a pump fills the
//! picker when the answer lands. Split from `lazy.rs` so both files stay under the line budget.

use std::time::{Duration, Instant};

use perch_ui::Shell;
use rune::{Env, Runtime};

use crate::lazy::gated;

#[test]
fn opening_settings_does_not_wait_and_a_pump_fills_the_picker() {
    let data = tempfile::tempdir().unwrap();
    let worktree = tempfile::tempdir().unwrap();
    let (app, _gate, open) = gated(&data, &worktree);
    let mut shell = Shell::new(app.metas(), 0).with_live(app);

    let started = Instant::now();
    shell.toggle_settings();
    assert!(
        started.elapsed() < Duration::from_millis(250),
        "the pane opening waited on the provider: {:?}",
        started.elapsed()
    );
    assert!(
        shell
            .settings_panel()
            .is_some_and(|panel| panel.models_loading()),
        "the picker says a listing is on its way"
    );

    let mut runtime = Runtime::new(
        shell,
        perch_ui::shell::view,
        (1280.0, 820.0),
        Env::new(true, 1.0),
    );
    runtime.frame(0.0);
    let tree = runtime.accessibility_json().to_string();
    assert!(
        tree.contains("asking the provider for its models"),
        "the open pane words the wait rather than looking empty:\n{tree}"
    );

    open.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut changed = false;
    while Instant::now() < deadline
        && runtime
            .state
            .settings_panel()
            .is_some_and(|panel| panel.models_loading())
    {
        changed |= runtime.state.pump(Duration::ZERO);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        !runtime
            .state
            .settings_panel()
            .is_some_and(|panel| panel.models_loading()),
        "the pump should have taken the answer"
    );
    assert!(changed, "taking the answer is a change the frame draws");
    let panel = runtime.state.settings_panel().expect("the panel is open");
    assert_eq!(
        panel.listed_models(),
        3,
        "the pump put every listed model in the picker"
    );
    // A panel with no saved model opens unfolded; one over a chosen model starts folded so it
    // cannot fill the pane, and this test is about the rows.
    if !runtime
        .state
        .settings_panel()
        .is_some_and(|panel| panel.models_open())
    {
        runtime.state.toggle_models();
    }
    // A zero-length frame does not re-run the view, so the frame that shows the filled list has
    // time on it, as a real one always does.
    runtime.frame(1.0 / 60.0);
    let tree = runtime.accessibility_json().to_string();
    for id in ["vendor/wide", "vendor/narrow", "vendor/silent"] {
        assert!(
            tree.contains(id),
            "the filled list is missing {id}:\n{tree}"
        );
    }
    assert!(
        !tree.contains("asking the provider for its models"),
        "the wait is gone once it answered:\n{tree}"
    );
}
