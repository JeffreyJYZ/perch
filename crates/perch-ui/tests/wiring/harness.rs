//! The window laid out through Rune, a fake editor, and the press a test makes on a key.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use perch_store::SessionMeta;
use perch_ui::Shell;
use perch_ui::files::Editor;
use perch_ui::shell::window_key;
use rune::{Env, Key, KeyEvent, Mods, Runtime};
use serde_json::Value;
use tempfile::TempDir;

/// The window the tests lay out, in points: the same size the binary opens.
pub const WINDOW: (f32, f32) = (1280.0, 820.0);

/// An editor that records what it was asked to open, and never runs anything.
#[derive(Default)]
pub struct Fake {
    pub opened: Mutex<Vec<PathBuf>>,
    pub refusal: Option<String>,
}

impl Fake {
    pub fn opened(&self) -> Vec<PathBuf> {
        self.opened.lock().unwrap().clone()
    }
}

impl Editor for Fake {
    fn open_in_zed(&self, path: &Path) -> Result<(), String> {
        self.opened.lock().unwrap().push(path.to_path_buf());
        match &self.refusal {
            Some(words) => Err(words.clone()),
            None => Ok(()),
        }
    }
}

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

/// A throwaway repository holding `files`, so the panel reads a real worktree.
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
        std::fs::write(dir.path().join(path), contents).unwrap();
    }
    dir
}

/// Stages everything and commits it, with identity and signing forced off so no global
/// configuration can change the outcome.
pub fn commit_all(repo: &TempDir) {
    for args in [
        ["add", "--all"].as_slice(),
        [
            "-c",
            "user.email=perch-test@example.com",
            "-c",
            "user.name=perch test",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "add",
        ]
        .as_slice(),
    ] {
        let status = Command::new("git")
            .args(args)
            .current_dir(repo.path())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }
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

/// The drawn box of `key`: `[x, y, width, height]` in window points.
fn bounds(runtime: &Runtime<Shell>, key: &str) -> [f32; 4] {
    let tree = runtime.accessibility_json();
    let node = find(&tree, key).unwrap_or_else(|| panic!("no {key} in {tree}"));
    let box_ = &node["bounds"];
    [
        number(&box_[0]),
        number(&box_[1]),
        number(&box_[2]),
        number(&box_[3]),
    ]
}

fn number(value: &Value) -> f32 {
    value.as_f64().unwrap_or(0.0) as f32
}

/// A click in the middle of `key`, which is what focuses a control or presses a row.
pub fn click(runtime: &mut Runtime<Shell>, key: &str) {
    let [x, y, w, h] = bounds(runtime, key);
    runtime.pointer_down(x + w / 2.0, y + h / 2.0);
    runtime.pointer_up(x + w / 2.0, y + h / 2.0);
    // A press is dispatched by the next frame, and a real one always has time on it.
    runtime.frame(1.0 / 60.0);
}

/// Everything the tree says, so a test can assert on a word the window draws.
pub fn words(runtime: &Runtime<Shell>) -> String {
    runtime.accessibility_json().to_string()
}

/// A laid-out window whose keys go through the binary's own map.
pub fn window(shell: Shell) -> Runtime<Shell> {
    let mut runtime = Runtime::new(shell, perch_ui::shell::view, WINDOW, Env::new(true, 2.0));
    runtime.on_key = Some(|shell: &mut Shell, ev, tx| window_key(shell, ev, tx));
    runtime.frame(0.0);
    runtime
}

pub fn enter() -> KeyEvent {
    KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    }
}
