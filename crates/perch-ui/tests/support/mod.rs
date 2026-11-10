//! The window the panel tests lay out, and the plugin fixtures they load into it.
//!
//! Not a test binary itself: only `.rs` files directly under `tests/` are, and each of those takes
//! the part of this it needs.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use perch_plugins::{Loaded, PanelEntry};
use perch_store::SessionMeta;
use perch_ui::Shell;
use perch_ui::shell::{Live, Panels, Waiting};
use rune::{Env, Key, KeyEvent, Mods, Runtime};
use serde_json::Value;
use tempfile::TempDir;

/// The window the tests lay out, in points: the same size the binary opens.
pub const WINDOW: (f32, f32) = (1280.0, 820.0);

/// The checked-in plugin fixtures live beside the sibling crate.
pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../perch-plugins/fixtures")
}

/// A throwaway plugin directory holding copies of `files`, named `(from, to)`.
pub fn dir_with(files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (from, to) in files {
        std::fs::copy(fixtures().join(from), dir.path().join(to)).unwrap();
    }
    dir
}

/// The app behind the window: the plugins and nothing else. A panel is all these tests read, so
/// the transcript, the prompt and the turn machinery stay at their defaults.
pub struct App {
    pub loaded: Loaded,
    /// When set, every press is refused with these words instead of reaching a plugin. It stands
    /// in for a seam that cannot hand a press over — the app refuses one for a control the panel
    /// no longer declares — so the column's refusal can be read without a race.
    pub refuse: Option<String>,
}

impl App {
    /// The app over a directory of plugins.
    pub fn over(dir: &Path) -> App {
        App {
            loaded: Loaded::open(dir, perch_tools::ToolRegistry::builtin()),
            refuse: None,
        }
    }

    /// The same app, refusing every press with `words`.
    pub fn refusing(dir: &Path, words: &str) -> App {
        App {
            refuse: Some(words.to_string()),
            ..App::over(dir)
        }
    }
}

impl Panels for App {
    fn panels(&self) -> Vec<PanelEntry> {
        self.loaded.panels()
    }

    fn plugin_words(&self) -> Vec<String> {
        self.loaded.words()
    }

    fn panel_action(
        &mut self,
        plugin: &str,
        control: &str,
        value: Option<String>,
    ) -> std::result::Result<(), String> {
        if let Some(refusal) = &self.refuse {
            return Err(refusal.clone());
        }
        self.loaded.panel_action(plugin, control, value.as_deref())
    }
}

impl Live for App {
    fn pump(&mut self, _: Duration) -> bool {
        false
    }
    fn transcript(&self) -> Option<&perch_ui::Transcript> {
        None
    }
    fn waiting(&self) -> Waiting {
        Waiting::Nothing
    }
    fn busy(&self) -> bool {
        false
    }
    fn prompt(&mut self, _: String) -> std::result::Result<(), String> {
        Ok(())
    }
    fn cancel(&mut self) {}
    fn approve(&mut self) {}
    fn deny(&mut self) {}
    fn pick(&mut self, _: usize) {}
    fn switch(&mut self, _: usize) {}
}

/// The window over `app`, laid out once.
pub fn window(app: App) -> Runtime<Shell<App>> {
    let meta = SessionMeta {
        id: "s".to_string(),
        title: "s".to_string(),
        worktree: PathBuf::from("/work"),
        branch: "dev".to_string(),
        model: String::new(),
        created: 0,
    };
    let mut runtime = Runtime::new(
        Shell::new(vec![meta], 0).with_live(app),
        perch_ui::shell::view,
        WINDOW,
        Env::new(true, 2.0),
    );
    runtime.frame(0.0);
    runtime
}

/// Everything the tree says, so a test can assert on a word the window draws.
pub fn tree(runtime: &Runtime<Shell<App>>) -> String {
    runtime.accessibility_json().to_string()
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

/// The box `key` was laid out in, as `[x, y, width, height]`.
pub fn bounds(runtime: &Runtime<Shell<App>>, key: &str) -> [f32; 4] {
    let tree = runtime.accessibility_json();
    let node = find(&tree, key).unwrap_or_else(|| panic!("no {key} in {tree}"));
    let rect = &node["bounds"];
    [
        number(&rect[0]),
        number(&rect[1]),
        number(&rect[2]),
        number(&rect[3]),
    ]
}

fn number(value: &Value) -> f32 {
    value.as_f64().unwrap_or(0.0) as f32
}

/// A click in the middle of `key`, which is what presses a button or focuses a field.
pub fn click(runtime: &mut Runtime<Shell<App>>, key: &str) {
    let [x, y, w, h] = bounds(runtime, key);
    runtime.pointer_down(x + w / 2.0, y + h / 2.0);
    runtime.pointer_up(x + w / 2.0, y + h / 2.0);
    // A press is dispatched by the next frame, and a real one always has time on it.
    runtime.frame(1.0 / 60.0);
}

/// One press of `key`, followed by the frame that runs its handler.
pub fn press(runtime: &mut Runtime<Shell<App>>, key: Key) {
    runtime.key(KeyEvent {
        key,
        mods: Mods::default(),
    });
    runtime.frame(1.0 / 60.0);
}

/// The Return a field submits on.
pub fn enter(runtime: &mut Runtime<Shell<App>>) {
    press(runtime, Key::Enter);
}

/// Types `text` into whatever has focus, one press per character.
pub fn type_text(runtime: &mut Runtime<Shell<App>>, text: &str) {
    for ch in text.chars() {
        press(runtime, Key::Text(ch.to_string()));
    }
}
