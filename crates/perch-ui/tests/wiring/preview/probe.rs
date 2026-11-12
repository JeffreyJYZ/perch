//! What the preview's tests read off a laid-out window: a box, an overlap, a press.

use std::path::{Path, PathBuf};

use perch_ui::Shell;
use rune::{Key, KeyEvent, Mods, Runtime};

use crate::harness::{find, words};

/// The box `key` is drawn in: `[x, y, width, height]` in window points.
pub fn bounds(runtime: &Runtime<Shell>, key: &str) -> [f32; 4] {
    let tree = runtime.accessibility_json();
    let node = find(&tree, key).unwrap_or_else(|| panic!("no {key} in {tree}"));
    let rect = &node["bounds"];
    [
        rect[0].as_f64().unwrap_or(0.0) as f32,
        rect[1].as_f64().unwrap_or(0.0) as f32,
        rect[2].as_f64().unwrap_or(0.0) as f32,
        rect[3].as_f64().unwrap_or(0.0) as f32,
    ]
}

/// Whether two boxes share any pixel at all.
pub fn shares_pixels(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}

/// The keys of `parent`'s direct children, so a test can walk the regions it stacks.
pub fn child_keys(runtime: &Runtime<Shell>, parent: &str) -> Vec<String> {
    let tree = runtime.accessibility_json();
    let node = find(&tree, parent).unwrap_or_else(|| panic!("no {parent} in {tree}"));
    node["children"]
        .as_array()
        .map(|children| {
            children
                .iter()
                .filter_map(|child| child["key"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// The file the tree has selected, as the panel resolved it.
pub fn selection(runtime: &Runtime<Shell>) -> Option<PathBuf> {
    runtime
        .state
        .files()
        .and_then(perch_ui::FilePanel::selected_path)
        .map(Path::to_path_buf)
}

/// A bare Escape, followed by the frame that answers it, and everything the window now says.
pub fn escape(runtime: &mut Runtime<Shell>) -> String {
    runtime.key(KeyEvent {
        key: Key::Escape,
        mods: Mods::default(),
    });
    runtime.frame(1.0 / 60.0);
    words(runtime)
}

/// Whether the centre column is showing a preview at all.
pub fn preview_shown(runtime: &Runtime<Shell>) -> bool {
    find(&runtime.accessibility_json(), "preview").is_some()
}
