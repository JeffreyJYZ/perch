//! The window laid out, plus the three ways a test reads its accessibility tree.

use std::path::PathBuf;
use std::process::Command;

use perch_store::SessionMeta;
use perch_ui::Shell;
use rune::{Env, Runtime};
use serde_json::Value;
use tempfile::TempDir;

/// The window the tests lay out, in points: the same size the binary opens.
pub const WINDOW: (f32, f32) = (1280.0, 820.0);

pub fn meta(title: &str) -> SessionMeta {
    SessionMeta {
        id: title.to_string(),
        title: title.to_string(),
        worktree: PathBuf::from("/work"),
        branch: "dev".to_string(),
        model: String::new(),
        created: 0,
    }
}

/// A throwaway repository holding `files`, so the file panel reads a real worktree.
pub fn temp_repo_with(files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    assert!(
        status.success(),
        "git init failed in {}",
        dir.path().display()
    );
    for (path, contents) in files {
        let path = dir.path().join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }
    dir
}

/// Lays `shell` out at [`WINDOW`] and returns its accessibility tree.
pub fn tree_of(shell: Shell) -> Value {
    let mut runtime = Runtime::new(shell, perch_ui::shell::view, WINDOW, Env::new(true, 2.0));
    runtime.frame(0.0);
    runtime.accessibility_json()
}

/// The node the app keyed `key`, wherever it sits.
pub fn find_key<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    if node.get("key").and_then(Value::as_str) == Some(key) {
        return Some(node);
    }
    let children = node.get("children").and_then(Value::as_array)?;
    children.iter().find_map(|child| find_key(child, key))
}

/// Every word `node`'s subtree says out loud: names, values and placeholders.
pub fn words(node: &Value, out: &mut Vec<String>) {
    for field in ["name", "value", "placeholder", "description"] {
        if let Some(text) = node.get(field).and_then(Value::as_str) {
            out.push(text.to_owned());
        }
    }
    for child in node
        .get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        words(child, out);
    }
}

/// Whether `node`'s subtree says `needle` anywhere.
pub fn says(node: &Value, needle: &str) -> bool {
    let mut out = Vec::new();
    words(node, &mut out);
    out.iter().any(|text| text.contains(needle))
}

/// The drawn box of `node`: `[x, y, width, height]` in window points.
pub fn bounds(node: &Value) -> [f64; 4] {
    let box_ = node
        .get("bounds")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("the node has no bounds: {node}"));
    [0, 1, 2, 3].map(|i| box_[i].as_f64().expect("a bound is a number"))
}
