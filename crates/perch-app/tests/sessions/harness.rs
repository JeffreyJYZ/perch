//! Two worktrees on disk, the window laid out over them, and the press a row takes.

use std::path::PathBuf;

use perch_app::{AppState, ProviderDefaults};
use perch_files::Workspace;
use perch_ui::Shell;
use rune::{Env, Runtime};
use serde_json::Value;
use tempfile::TempDir;

/// The window the tests lay out, in points: the same size the binary opens.
const WINDOW: (f32, f32) = (1280.0, 820.0);

/// Tests must not read the machine they run on.
///
/// A real `~/.config/opencode/AGENTS.md` joins every session's system message, so a machine's own
/// notes would change what the column says and a test would pass here and fail there. This points
/// the home at an empty directory once per test process; `PERCH_HOME` is the seam
/// `perch_agent::instructions` documents. Called from [`defaults`], which every app in this
/// target uses before it opens.
fn hermetic() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let home = std::env::temp_dir().join("perch-tests-home");
        let _ = std::fs::create_dir_all(&home);
        // Safety: it is set once per process, before any app is built, and only under test.
        unsafe { std::env::set_var("PERCH_HOME", &home) };
    });
}

pub fn defaults() -> ProviderDefaults {
    hermetic();
    ProviderDefaults {
        base_url: "http://127.0.0.1:9".to_string(),
        api_key: "test-key".to_string(),
        model: "test/model".to_string(),
    }
}

/// A throwaway data directory and two worktrees named `one` and `two`, so a row's title is the
/// directory's own name rather than a random temporary one.
pub fn fixture() -> (TempDir, TempDir, PathBuf, PathBuf) {
    let data = tempfile::tempdir().unwrap();
    let roots = tempfile::tempdir().unwrap();
    let one = roots.path().join("one");
    let two = roots.path().join("two");
    std::fs::create_dir_all(&one).unwrap();
    std::fs::create_dir_all(&two).unwrap();
    (data, roots, one, two)
}

/// The window over `app`, laid out and drawn once, with the binary's own key map.
pub fn laid_out(app: AppState) -> Runtime<Shell<AppState>> {
    // The same construction the binary uses: the file panel opens on the session that is shown.
    let (metas, active) = (app.metas(), app.active());
    let shell = match metas
        .get(active)
        .and_then(|meta| Workspace::open(&meta.worktree).ok())
    {
        Some(workspace) => Shell::with_workspace(metas, active, workspace),
        None => Shell::new(metas, active),
    };
    let mut runtime = Runtime::new(
        shell.with_live(app),
        perch_ui::shell::view,
        WINDOW,
        Env::new(true, 2.0),
    );
    runtime.on_key =
        Some(|shell: &mut Shell<AppState>, ev, tx| perch_ui::shell::window_key(shell, ev, tx));
    runtime.frame(0.0);
    runtime
}

/// The key of the session row whose row names `title`.
pub fn session_key(runtime: &Runtime<Shell<AppState>>, title: &str) -> String {
    let tree = runtime.accessibility_json();
    fn walk(node: &Value, title: &str) -> Option<String> {
        let key = node["key"].as_str().unwrap_or_default();
        let name = node["name"].as_str().unwrap_or_default();
        if key.starts_with("session-") && name.starts_with(title) {
            return Some(key.to_owned());
        }
        node["children"]
            .as_array()?
            .iter()
            .find_map(|child| walk(child, title))
    }
    walk(&tree, title).unwrap_or_else(|| panic!("no session row for {title}: {tree}"))
}

/// Every word the sidebar says.
pub fn sidebar(runtime: &Runtime<Shell<AppState>>) -> String {
    let tree = runtime.accessibility_json();
    find(&tree, "left-column")
        .map(ToString::to_string)
        .unwrap_or_default()
}

/// The node keyed `key`, wherever it sits in the tree.
pub fn find<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    if node["key"] == key {
        return Some(node);
    }
    node["children"]
        .as_array()?
        .iter()
        .find_map(|child| find(child, key))
}

/// A click in the middle of `key`, which is what a row's or a button's press is.
pub fn click(runtime: &mut Runtime<Shell<AppState>>, key: &str) {
    let tree = runtime.accessibility_json();
    let node = find(&tree, key).unwrap_or_else(|| panic!("no {key} in {tree}"));
    let box_ = &node["bounds"];
    let [x, y, w, h] = [0, 1, 2, 3].map(|i| box_[i].as_f64().unwrap_or(0.0) as f32);
    runtime.pointer_down(x + w / 2.0, y + h / 2.0);
    runtime.pointer_up(x + w / 2.0, y + h / 2.0);
    // A press is dispatched by the next frame, and a real one always has time on it.
    runtime.frame(1.0 / 60.0);
}
