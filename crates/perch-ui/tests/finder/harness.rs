//! The window the finder tests lay out, and the presses they make on it.
//!
//! Not a test binary itself: `tests/finder/main.rs` takes the part of this each module needs.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use perch_files::Workspace;
use perch_store::SessionMeta;
use perch_ui::Shell;
use perch_ui::shell::window_key;
use rune::{Env, Key, KeyEvent, Mods, Runtime};
use serde_json::Value;
use tempfile::TempDir;

/// The window the tests lay out, in points: the same size the binary opens.
const WINDOW: (f32, f32) = (1280.0, 820.0);

/// A worktree holding `files`, each written with a line so it can be previewed.
pub fn worktree(files: &[&str]) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for path in files {
        let full = dir.path().join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(&full, format!("// {path}\n")).unwrap();
    }
    dir
}

/// The window over `dir`, laid out once with the binary's key map.
pub fn window(dir: &Path) -> Runtime<Shell> {
    let workspace = Workspace::open(dir).unwrap();
    let meta = SessionMeta {
        id: "s".to_string(),
        title: "s".to_string(),
        worktree: dir.to_path_buf(),
        branch: "dev".to_string(),
        model: String::new(),
        created: 0,
    };
    let shell = Shell::with_workspace(vec![meta], 0, workspace);
    let mut runtime = Runtime::new(shell, perch_ui::shell::view, WINDOW, Env::new(true, 2.0));
    runtime.on_key = Some(|shell: &mut Shell, ev, tx| window_key(shell, ev, tx));
    runtime.frame(0.0);
    runtime
}

/// ⌘P, and the frame that commits the overlay.
pub fn chord(runtime: &mut Runtime<Shell>) {
    runtime.key(KeyEvent {
        key: Key::Command('p'),
        mods: Mods {
            cmd: true,
            ..Mods::default()
        },
    });
    runtime.frame(0.0);
}

/// One press, followed by the frame that runs its handler.
pub fn press(runtime: &mut Runtime<Shell>, key: Key) {
    runtime.key(KeyEvent {
        key,
        mods: Mods::default(),
    });
    runtime.frame(0.0);
}

/// Types `text` one press per character, which is what a person does.
pub fn type_text(runtime: &mut Runtime<Shell>, text: &str) {
    for ch in text.chars() {
        press(runtime, Key::Text(ch.to_string()));
    }
}

/// Everything the tree says, so a test can assert on a word the overlay draws.
pub fn words(runtime: &Runtime<Shell>) -> String {
    runtime.accessibility_json().to_string()
}

/// The node keyed `key`, wherever it sits in the tree.
fn find<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    if node["key"] == key {
        return Some(node);
    }
    node["children"]
        .as_array()?
        .iter()
        .find_map(|child| find(child, key))
}

/// A click in the middle of `key`, which focuses a field or presses a row.
pub fn click(runtime: &mut Runtime<Shell>, key: &str) {
    let tree = runtime.accessibility_json();
    let node = find(&tree, key).unwrap_or_else(|| panic!("no {key} in {tree}"));
    let rect = &node["bounds"];
    let [x, y, w, h] = [0, 1, 2, 3].map(|index| rect[index].as_f64().unwrap_or(0.0) as f32);
    runtime.pointer_down(x + w / 2.0, y + h / 2.0);
    runtime.pointer_up(x + w / 2.0, y + h / 2.0);
    runtime.frame(1.0 / 60.0);
}

/// The tree row that names `path`, so a test presses what a person would press.
pub fn row_of(runtime: &Runtime<Shell>, path: &Path) -> usize {
    let rows = runtime.state.files().expect("the worktree is open").rows();
    rows.iter()
        .position(|row| row.path == path)
        .unwrap_or_else(|| panic!("no tree row for {}", path.display()))
}

/// `path` as the shell's own worktree resolves it.
pub fn in_worktree(runtime: &Runtime<Shell>, path: &str) -> PathBuf {
    runtime.state.workspace().unwrap().root().join(path)
}
