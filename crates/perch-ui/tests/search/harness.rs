//! The window the search tests lay out, and the presses they make on it.
//!
//! Not a test binary itself: `tests/search/main.rs` takes the part of this each module needs.

#![allow(dead_code)]

use std::path::PathBuf;

use perch_store::SessionMeta;
use perch_ui::Shell;
use perch_ui::shell::window_key;
use rune::{Env, Key, KeyEvent, Mods, Runtime};
use serde_json::Value;

/// The window the tests lay out, in points: the same size the binary opens.
pub const WINDOW: (f32, f32) = (1280.0, 820.0);

/// A session as the list reads it: its title, its worktree and its branch.
pub fn session(title: &str, worktree: &str, branch: &str) -> SessionMeta {
    SessionMeta {
        id: title.to_string(),
        title: title.to_string(),
        worktree: PathBuf::from(worktree),
        branch: branch.to_string(),
        model: String::new(),
        created: 0,
    }
}

/// The window over `sessions`, active `active`, laid out once with the binary's key map.
pub fn window(sessions: Vec<SessionMeta>, active: usize) -> Runtime<Shell> {
    let shell = Shell::idle(sessions, active);
    let mut runtime = Runtime::new(shell, perch_ui::shell::view, WINDOW, Env::new(true, 2.0));
    runtime.on_key = Some(|shell: &mut Shell, ev, tx| window_key(shell, ev, tx));
    runtime.frame(0.0);
    runtime
}

/// A chord with Command held, as the window's own shortcuts read it.
pub fn chord(runtime: &mut Runtime<Shell>, key: char) {
    runtime.key(KeyEvent {
        key: Key::Command(key),
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

/// Everything the tree says, so a test can assert on a word the window draws.
pub fn words(runtime: &Runtime<Shell>) -> String {
    runtime.accessibility_json().to_string()
}

/// The node keyed `key`, wherever it sits in the tree.
pub fn node<'a>(tree: &'a Value, key: &str) -> Option<&'a Value> {
    if tree["key"] == key {
        return Some(tree);
    }
    tree["children"]
        .as_array()?
        .iter()
        .find_map(|child| node(child, key))
}

/// The bounds of the element keyed `key`, as the tree reports them: x, y, width, height.
pub fn bounds(runtime: &Runtime<Shell>, key: &str) -> [f32; 4] {
    let tree = runtime.accessibility_json();
    let found = node(&tree, key).unwrap_or_else(|| panic!("no {key} in the tree"));
    let rect = &found["bounds"];
    [0, 1, 2, 3].map(|index| rect[index].as_f64().unwrap_or(0.0) as f32)
}

/// A click in the middle of `key`, which focuses a field or presses a row.
pub fn click(runtime: &mut Runtime<Shell>, key: &str) {
    let [x, y, w, h] = bounds(runtime, key);
    runtime.pointer_down(x + w / 2.0, y + h / 2.0);
    runtime.pointer_up(x + w / 2.0, y + h / 2.0);
    runtime.frame(1.0 / 60.0);
}

/// A tool call as the transcript records one.
pub fn call(id: &str, name: &str) -> perch_agent::ToolCall {
    perch_agent::ToolCall {
        id: id.to_string(),
        name: name.to_string(),
        args: serde_json::json!({ "path": "src/main.rs" }),
    }
}

/// A window over one session and the transcript `build` writes into it.
pub fn over(build: impl FnOnce(&mut Shell)) -> Runtime<Shell> {
    let mut runtime = window(vec![session("s", "/work/s", "dev")], 0);
    runtime.update(|shell, _| build(shell));
    runtime.frame(0.0);
    runtime
}

/// What the transcript header says about how far up the column is scrolled.
pub fn header(runtime: &Runtime<Shell>) -> String {
    let tree = runtime.accessibility_json();
    node(&tree, "transcript-scroll-words")
        .and_then(|words| words["name"].as_str())
        .unwrap_or_default()
        .to_owned()
}

/// The transcript column's own scroll offset, in content coordinates: a bar opening or closing
/// above it changes how much there is to scroll, never where the column sits.
pub fn scroll_offset(runtime: &Runtime<Shell>) -> f64 {
    let tree = runtime.accessibility_json();
    node(&tree, "transcript-scroll").map_or(0.0, |el| el["scroll"][1].as_f64().unwrap_or(0.0))
}

/// Whether the element keyed `key` is drawn inside the transcript's viewport.
pub fn in_view(runtime: &Runtime<Shell>, key: &str) -> bool {
    let [_, top, _, height] = bounds(runtime, "transcript-scroll");
    let [_, hit_top, _, hit_height] = bounds(runtime, key);
    hit_top >= top - 1.0 && hit_top + hit_height <= top + height + 1.0
}
