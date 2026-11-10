//! Reloading the tools without a restart: a plugin that arrives answers, an edited one answers
//! differently, and a deleted one is gone for good.
//!
//! Every step here drives `reload()` directly, so the test proves the rebuild and the swap rather
//! than the clock; the watcher is what turns a save into that call and has its own test. The
//! provider is stated and the secret store is in memory, so nothing here reaches the keychain or a
//! model.

mod support;

use std::path::{Path, PathBuf};

use perch_app::{AppState, ProviderDefaults, Sources};
use perch_store::secret::MemorySecrets;
use perch_tools::ToolCtx;
use serde_json::json;
use support::fixtures;

fn app(dir: &Path) -> AppState {
    AppState::open_with_secrets(
        dir,
        ProviderDefaults {
            base_url: "http://127.0.0.1:1/v1".to_string(),
            api_key: String::new(),
            model: "test/model".to_string(),
        },
        Box::new(MemorySecrets::default()),
    )
    .expect("the app opens over the data directory")
}

fn plugin_dir(dir: &Path) -> PathBuf {
    dir.join(perch_app::settings::PLUGINS)
}

fn names(app: &AppState) -> Vec<String> {
    app.tools().into_iter().map(|spec| spec.name).collect()
}

#[test]
fn a_rebuild_starts_from_empty_so_a_deleted_plugin_stays_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let plugins = dir.path().join("plugins");
    std::fs::create_dir_all(&plugins).unwrap();
    let mut sources = Sources::open(plugins.clone(), &[]).expect("no servers, no plugins");
    let ctx = ToolCtx {
        root: plugins.clone(),
    };

    assert!(
        !sources
            .registry()
            .specs()
            .iter()
            .any(|spec| spec.name == "echo"),
        "a tool exists before any plugin was installed"
    );

    // Added: the tool answers with no app and no restart in between.
    std::fs::copy(
        fixtures().join("counter.wasm"),
        plugins.join("counter.wasm"),
    )
    .unwrap();
    sources.rebuild(&[]);
    assert_eq!(
        sources
            .registry()
            .call("echo", &json!({"text": "hi"}), &ctx)
            .expect("the plugin answers after a rebuild")
            .text,
        "hi"
    );

    // A file that will not load is one line, and the plugin beside it still answers.
    std::fs::write(plugins.join("broken.wasm"), b"not a component").unwrap();
    let words = sources.rebuild(&[]);
    assert!(
        words.iter().any(|words| words.contains("broken.wasm")),
        "no word named the file that would not load: {words:?}"
    );
    assert_eq!(
        sources
            .registry()
            .call("echo", &json!({"text": "still here"}), &ctx)
            .unwrap()
            .text,
        "still here"
    );

    // Deleted: the tool goes, and a second rebuild has nothing to resurrect it from.
    std::fs::remove_file(plugins.join("counter.wasm")).unwrap();
    sources.rebuild(&[]);
    assert!(
        !sources
            .registry()
            .specs()
            .iter()
            .any(|spec| spec.name == "echo"),
        "a deleted plugin's tool is still offered"
    );
    sources.rebuild(&[]);
    assert!(
        !sources
            .registry()
            .specs()
            .iter()
            .any(|spec| spec.name == "echo"),
        "a deleted plugin came back on the next rebuild"
    );
}

#[test]
fn a_plugin_can_be_added_edited_and_deleted_under_a_running_app() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plugin_dir(dir.path())).unwrap();
    let mut app = app(dir.path());
    let file = plugin_dir(dir.path()).join("counter.wasm");

    // Added: the app's own tools and the dispatch path see it after one reload.
    std::fs::copy(fixtures().join("counter.wasm"), &file).unwrap();
    assert!(app.reload(), "the settings file could not be read");
    assert!(
        names(&app).contains(&"echo".to_string()),
        "the new plugin's tool is not offered: {:?}",
        names(&app)
    );
    assert_eq!(
        app.call_tool("echo", &json!({"text": "perch"}))
            .expect("the plugin answers without a restart")
            .text,
        "perch"
    );

    // Edited: the same file, a different component, so the tools change and the old one is gone.
    std::fs::copy(fixtures().join("trap-panel/trap-panel.wasm"), &file).unwrap();
    app.reload();
    assert!(
        !names(&app).contains(&"echo".to_string()),
        "the edited plugin's old tool is still offered: {:?}",
        names(&app)
    );
    assert!(
        names(&app).contains(&"fine".to_string()),
        "the edited plugin's new tool is not offered: {:?}",
        names(&app)
    );
    assert_eq!(
        app.call_tool("fine", &json!({})).unwrap().text,
        "fine",
        "the edited plugin answers with its new behaviour"
    );

    // Deleted: gone from what is offered and from what dispatches, twice over.
    std::fs::remove_file(&file).unwrap();
    app.reload();
    app.reload();
    assert!(
        !names(&app).contains(&"fine".to_string()),
        "the deleted plugin's tool is still offered: {:?}",
        names(&app)
    );
    assert!(
        app.call_tool("fine", &json!({})).is_err(),
        "a deleted plugin still answers"
    );
}
