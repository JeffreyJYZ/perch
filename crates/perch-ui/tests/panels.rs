//! The plugin column, through the window's own layout and the accessibility tree.
//!
//! A panel is data the host decoded, so the column is proved by what the tree says: the plugin's
//! region, its title, its line of text — and its absence when no plugin is loaded, because a window
//! without plugins is the window this app shipped before it had any.

mod support;

use support::{App, dir_with, find, tree, window};

#[test]
fn a_panel_column_is_composed_from_the_host_and_gone_without_a_plugin() {
    // No plugin at all: no column, no title, no region. The window is the window it always was.
    let empty = tempfile::tempdir().unwrap();
    let idle = window(App::over(empty.path()));
    let blank = idle.accessibility_json();
    assert!(
        find(&blank, "plugins").is_none(),
        "a window with no plugin drew a plugin column: {blank}"
    );
    assert!(
        !tree(&idle).contains("Counter"),
        "a panel was drawn from no plugin: {}",
        tree(&idle)
    );

    // One plugin: its panel is the host's model, drawn in a column of its own beside the window.
    let dir = dir_with(&[("counter.wasm", "counter.wasm")]);
    let runtime = window(App::over(dir.path()));
    let drawn = tree(&runtime);
    assert!(
        find(&runtime.accessibility_json(), "plugins").is_some(),
        "the column has no region of its own: {drawn}"
    );
    for word in ["Plugins", "Counter", "count: 0"] {
        assert!(
            drawn.contains(word),
            "the column is missing {word}: {drawn}"
        );
    }

    // One panel per plugin: two plugins load, and the one that declares a panel is the one drawn.
    let both = dir_with(&[
        ("counter.wasm", "counter.wasm"),
        ("trap-panel/trap-panel.wasm", "trap-panel.wasm"),
    ]);
    let drawn = tree(&window(App::over(both.path())));
    assert!(
        drawn.contains("plugin-counter"),
        "the panel is not keyed by its plugin: {drawn}"
    );
    assert!(
        !drawn.contains("plugin-trap-panel"),
        "a panel that trapped was drawn anyway: {drawn}"
    );
}
