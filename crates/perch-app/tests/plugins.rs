//! The app's plugin load path: a directory of components, the registry their tools land in beside
//! the built-ins, the words for a file that would not load, and the release that takes a trapped
//! plugin's tools away.
//!
//! Nothing here needs a provider: a plugin is loaded, called and released without a turn running,
//! which is exactly what the window does at startup and again on every reload.

mod support;

use perch_app::Sources;
use perch_tools::ToolCtx;
use serde_json::json;
use support::{app, dir_with, fixtures, session};

fn sources(dir: &std::path::Path) -> Sources {
    Sources::open(dir.to_path_buf(), &[]).expect("a plugin directory opens without MCP servers")
}

#[test]
fn a_plugin_directory_registers_its_tools_beside_the_builtins() {
    let dir = dir_with(&[("counter.wasm", "counter.wasm")]);
    let sources = sources(dir.path());

    assert_eq!(
        sources.plugin_count(),
        1,
        "the fixture plugin did not load: {:?}",
        sources.words()
    );
    let names: Vec<String> = sources
        .registry()
        .specs()
        .into_iter()
        .map(|spec| spec.name)
        .collect();
    for builtin in ["bash", "read"] {
        assert!(
            names.contains(&builtin.to_string()),
            "the built-in {builtin} is missing from {names:?}"
        );
    }
    assert!(
        names.contains(&"echo".to_string()),
        "the plugin's tool is missing from {names:?}"
    );
    assert!(sources.words().is_empty(), "{:?}", sources.words());

    // One registry, one dispatch path: the plugin's tool answers exactly where `bash` does, and an
    // approval-gated built-in still asks.
    let ctx = ToolCtx {
        root: dir.path().to_path_buf(),
    };
    let out = sources
        .registry()
        .call("echo", &json!({"text": "perch"}), &ctx)
        .expect("the plugin tool ran");
    assert_eq!(out.text, "perch");
    assert!(
        sources.registry().needs_approval("bash").unwrap(),
        "the plugin load changed a built-in's policy"
    );
}

#[test]
fn a_component_that_will_not_load_is_words_and_the_app_still_opens() {
    let data = tempfile::tempdir().unwrap();
    let plugins = data.path().join("plugins");
    std::fs::create_dir_all(&plugins).unwrap();
    std::fs::copy(
        fixtures().join("counter.wasm"),
        plugins.join("counter.wasm"),
    )
    .unwrap();
    std::fs::write(plugins.join("broken.wasm"), b"not a component").unwrap();
    session(data.path());

    let sources = sources(&plugins);
    assert_eq!(
        sources.plugin_count(),
        1,
        "a broken file cost the plugin beside it: {:?}",
        sources.words()
    );
    assert!(
        sources
            .words()
            .iter()
            .any(|words| words.contains("broken.wasm")),
        "no line names the file that would not load: {:?}",
        sources.words()
    );

    // The window opens over the same data directory, with the plugin that did load.
    assert_eq!(app(data.path()).sessions().len(), 1);
}

#[test]
fn a_plugin_that_traps_is_released_and_named() {
    let dir = dir_with(&[("counter.wasm", "counter.wasm")]);
    let mut sources = sources(dir.path());
    let ctx = ToolCtx {
        root: dir.path().to_path_buf(),
    };

    assert!(
        sources
            .registry()
            .call("explode", &json!({}), &ctx)
            .is_err(),
        "the trapping fixture did not trap"
    );
    // A trap disables the plugin but does not touch the registry: the caller that holds the
    // registry releases it, which is the call the app makes on the next frame.
    assert!(
        sources
            .registry()
            .specs()
            .iter()
            .any(|spec| spec.name == "echo"),
        "the tool went before the release"
    );

    assert!(
        sources.settle(),
        "the app did not notice that a plugin had trapped"
    );
    for name in ["echo", "explode", "read_outside"] {
        assert!(
            !sources
                .registry()
                .specs()
                .iter()
                .any(|spec| spec.name == name),
            "{name} is still offered after release"
        );
    }
    assert!(
        sources.panels().is_empty(),
        "a disabled plugin still draws a panel"
    );
    assert!(
        sources
            .words()
            .iter()
            .any(|words| words.contains("counter") && words.contains("disabled")),
        "nothing says which plugin went: {:?}",
        sources.words()
    );
    assert!(
        sources.registry().needs_approval("bash").unwrap(),
        "the release touched a built-in"
    );
}
