//! What reaches a plugin: a session's events, and what a guest that traps costs it.
//!
//! Both are proved on the drawn column, because that is the only thing the window promises: the
//! counter's number moves when an event arrives, and a plugin that traps leaves its name in words
//! beside the panels that are still there.

mod support;

use perch_store::SessionEvent;
use support::{App, dir_with, tree, window};

#[test]
fn a_turn_end_and_a_tool_result_reach_the_plugin_and_the_drawn_column_counts_it() {
    let dir = dir_with(&[("counter.wasm", "counter.wasm")]);
    let mut runtime = window(App::over(dir.path()));
    assert!(tree(&runtime).contains("count: 0"), "{}", tree(&runtime));

    {
        let app = runtime.state.live_mut().expect("the app is attached");
        app.loaded.dispatch(&SessionEvent::TurnEnd {
            reason: "done".to_string(),
            tokens_in: 3,
            tokens_out: 4,
        });
        // A tool result reaches the guest too. The counter does not count them, so the assertion
        // is that it changes nothing rather than that it breaks the column.
        app.loaded.dispatch(&SessionEvent::ToolResult {
            id: "call-1".to_string(),
            ok: true,
            text: "ok".to_string(),
        });
    }
    // The dispatch happened outside any handler, so the runtime is told the state it reads moved.
    runtime.invalidate();
    runtime.frame(1.0 / 60.0);

    let drawn = tree(&runtime);
    assert!(
        drawn.contains("count: 1"),
        "the drawn column still shows the old panel: {drawn}"
    );
    assert!(drawn.contains("Counter"), "{drawn}");
}

#[test]
fn a_plugin_that_traps_leaves_the_column_saying_which_one() {
    let dir = dir_with(&[("trap-panel/trap-panel.wasm", "trap-panel.wasm")]);
    let mut runtime = window(App::over(dir.path()));

    let drawn = tree(&runtime);
    assert!(
        drawn.contains("plugin `trap-panel` was disabled"),
        "the column does not say which plugin went: {drawn}"
    );
    assert!(
        drawn.contains("Plugins"),
        "the column itself is still there: {drawn}"
    );

    // The tools leave the registry through the release the app runs after a call has failed.
    let app = runtime.state.live_mut().expect("the app is attached");
    assert_eq!(
        app.loaded.release(),
        1,
        "the plugin's one tool was not released"
    );
    assert!(
        !app.loaded
            .registry()
            .specs()
            .iter()
            .any(|spec| spec.name == "fine"),
        "a disabled plugin's tool is still offered"
    );
}
