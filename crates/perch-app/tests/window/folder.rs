//! The folder picker, driven with a scripted dialog: pressing Open folder makes a session on the
//! folder it answers with, and a folder a person picked is opened wherever it is.

use std::path::{Path, PathBuf};

use perch_app::AppState;
use perch_ui::Shell;
use rune::{Env, FileDialogKind, FileDialogs, Runtime, ScriptedFileDialog};
use serde_json::Value;

use crate::harness::defaults;

/// The window the binary opens, in points.
const WINDOW: (f32, f32) = (1280.0, 820.0);

/// An app with no session, over a throwaway data directory.
fn empty_app() -> (AppState, tempfile::TempDir) {
    let data = tempfile::tempdir().unwrap();
    let app = AppState::open_with(data.path(), defaults("http://127.0.0.1:9")).unwrap();
    (app, data)
}

/// A picker that answers every question with `path`, and never shows a panel.
fn picker(path: Option<PathBuf>) -> FileDialogs {
    FileDialogs::scripted([ScriptedFileDialog {
        kind: FileDialogKind::Folder,
        path,
    }])
}

/// A laid-out window over `shell`.
fn window(shell: Shell<AppState>) -> Runtime<Shell<AppState>> {
    let mut runtime = Runtime::new(shell, perch_ui::shell::view, WINDOW, Env::new(true, 2.0));
    runtime.frame(0.0);
    runtime
}

/// The node the app keyed `key`, wherever it sits.
fn find<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    if node.get("key").and_then(Value::as_str) == Some(key) {
        return Some(node);
    }
    let children = node.get("children").and_then(Value::as_array)?;
    children.iter().find_map(|child| find(child, key))
}

/// A click in the middle of `key`, which is what presses a button.
fn click(runtime: &mut Runtime<Shell<AppState>>, key: &str) {
    let tree = runtime.accessibility_json();
    let node = find(&tree, key).unwrap_or_else(|| panic!("no {key} in {tree}"));
    let box_ = node
        .get("bounds")
        .and_then(Value::as_array)
        .expect("bounds");
    let x = box_[0].as_f64().unwrap() as f32 + box_[2].as_f64().unwrap() as f32 / 2.0;
    let y = box_[1].as_f64().unwrap() as f32 + box_[3].as_f64().unwrap() as f32 / 2.0;
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    // A press is dispatched by the next frame, and a real one always has time on it.
    runtime.frame(1.0 / 60.0);
}

/// Everything the tree says, so a test can assert on a word the window draws.
fn words(runtime: &Runtime<Shell<AppState>>) -> String {
    runtime.accessibility_json().to_string()
}

#[test]
fn no_folder_then_open_folder_makes_a_session_on_the_scripted_path() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("README.md"), "# perch").unwrap();
    std::fs::write(repo.path().join("main.rs"), "fn main() {}").unwrap();
    let root = repo.path().canonicalize().unwrap();

    let (app, _data) = empty_app();
    assert!(app.metas().is_empty(), "the app opens with no session");
    let shell = Shell::new(app.metas(), 0)
        .with_live(app)
        .with_folder_dialogs(picker(Some(repo.path().to_path_buf())));

    let mut runtime = window(shell);
    let before = words(&runtime);
    assert!(before.contains("No folder open"), "{before}");
    assert!(find(&runtime.accessibility_json(), "open-folder").is_some());
    assert!(
        runtime.state.terminals().is_empty(),
        "a window with no folder spawns no shell"
    );

    click(&mut runtime, "open-folder");

    assert_eq!(
        runtime.state.tab_titles().len(),
        1,
        "the session list gained a row: {:?}",
        runtime.state.tab_titles()
    );
    assert_eq!(
        runtime.state.active_session().map(|meta| &meta.worktree),
        Some(&root),
        "the scripted folder became the session's worktree"
    );
    assert!(
        runtime
            .state
            .live()
            .is_some_and(|app| app.metas().len() == 1),
        "the app really opened the session, not just the row"
    );
    assert!(
        runtime.state.files().is_some(),
        "the file tree loaded for the picked folder"
    );
    let after = words(&runtime);
    for name in ["README.md", "main.rs"] {
        assert!(after.contains(name), "the tree shows {name}: {after}");
    }
    assert!(
        runtime.state.terminals().is_empty(),
        "a scripted pick never spawns a pty"
    );
}

#[test]
fn a_picked_root_or_home_is_opened_not_refused() {
    let (mut app, _data) = empty_app();
    let root = app
        .open_folder(Path::new("/"))
        .expect("a picked root opens");
    assert_eq!(app.metas()[root].worktree, PathBuf::from("/"));

    let mut expected = 1;
    // The person's own home is the same decision, and macOS asks for whatever walking it needs.
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        let index = app.open_folder(&home).expect("a picked home opens");
        assert_eq!(app.metas()[index].worktree, home.canonicalize().unwrap());
        expected += 1;
    }
    assert_eq!(app.metas().len(), expected);
}

#[test]
fn a_cancelled_picker_changes_nothing() {
    let (app, _data) = empty_app();
    let mut shell = Shell::new(Vec::new(), 0)
        .with_live(app)
        .with_folder_dialogs(picker(None));
    let mut tx = rune::Tx::default();
    shell.open_folder(&mut tx);

    assert!(
        shell.tab_titles().is_empty(),
        "a cancelled pick adds no row: {:?}",
        shell.tab_titles()
    );
    assert_eq!(shell.notice(), None, "cancelling is not a failure");
    assert!(shell.terminals().is_empty(), "and spawns no shell");
}
