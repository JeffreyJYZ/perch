//! A shell in a throwaway worktree, the keys the dock hands it, and the frame it publishes.

use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use perch_store::SessionMeta;
use perch_ui::Shell;
use perch_ui::shell::terminal_key;
use rune::{Env, Key, KeyEvent, Mods, Runtime, Tx};
use serde_json::Value;

/// The window the tests lay out, in points: the same size the binary opens.
pub const WINDOW: (f32, f32) = (1280.0, 820.0);
/// How long a test waits for the shell to say something.
pub const TIMEOUT: Duration = Duration::from_secs(15);

pub fn meta(worktree: &Path) -> SessionMeta {
    SessionMeta {
        id: worktree.display().to_string(),
        title: "dock".to_string(),
        worktree: worktree.to_path_buf(),
        branch: String::new(),
        model: String::new(),
        created: 0,
    }
}

/// Points this process's terminals at `/bin/sh`, once.
pub fn use_sh() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        // SAFETY: this binary's own tests are the only readers, they all want the same value, and
        // a pty is spawned after the variable is set.
        unsafe {
            std::env::set_var("SHELL", "/bin/sh");
        }
    });
}

/// Types `line` at the shell and presses Return, one key at a time through the dock's own map.
pub fn type_line(shell: &mut Shell, line: &str) {
    let mut tx = Tx::default();
    for character in line.chars() {
        let ev = KeyEvent {
            key: Key::Text(character.to_string()),
            mods: Mods::default(),
        };
        assert!(
            terminal_key(shell, &ev, &mut tx),
            "the dock hands {character:?} to the shell"
        );
    }
    let enter = KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    };
    assert!(
        terminal_key(shell, &enter, &mut tx),
        "Return reaches the shell"
    );
}

/// The text of the frame the dock would paint, all rows joined.
pub fn frame_text(shell: &Shell) -> String {
    let Some(frame) = shell.active_terminal().and_then(|tab| tab.frame()) else {
        return String::new();
    };
    perch_ui::paint_rows(frame)
        .iter()
        .map(|run| run.text.clone())
        .collect()
}

/// Pumps the window until `done` holds, or [`TIMEOUT`] runs out.
pub fn pump_until(shell: &mut Shell, done: impl Fn(&Shell) -> bool) -> bool {
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        if done(shell) {
            return true;
        }
        shell.pump(Duration::from_millis(5));
    }
    done(shell)
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

/// Whether `node`'s subtree says `needle` anywhere out loud.
pub fn says(node: &Value, needle: &str) -> bool {
    fn words(node: &Value, out: &mut Vec<String>) {
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
    let mut out = Vec::new();
    words(node, &mut out);
    out.iter().any(|text| text.contains(needle))
}
