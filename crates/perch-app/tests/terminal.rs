//! The dock against a real shell.
//!
//! A window cannot be drawn in a test, but the two things the dock promises can be: a spawned
//! pty's output arrives as a [`Frame`](perch_term::Frame) and reaches the grid the dock paints,
//! and a shell that has exited says so in words instead of leaving its last frame on screen as if
//! it were still printing.
//!
//! `SHELL` is pointed at `/bin/sh` for this test binary: the user's own shell reads their rc, and
//! a test of the dock must not depend on what a dotfile prints.

use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use perch_store::SessionMeta;
use perch_ui::Shell;
use perch_ui::shell::terminal_key;
use rune::{Env, Key, KeyEvent, Mods, Runtime, Tx};
use serde_json::Value;

/// The window the tests lay out, in points: the same size the binary opens.
const WINDOW: (f32, f32) = (1280.0, 820.0);
/// How long a test waits for the shell to say something.
const TIMEOUT: Duration = Duration::from_secs(15);

fn meta(worktree: &Path) -> SessionMeta {
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
fn use_sh() {
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
fn type_line(shell: &mut Shell, line: &str) {
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
fn frame_text(shell: &Shell) -> String {
    let Some(frame) = shell.active_terminal().and_then(|tab| tab.frame()) else {
        return String::new();
    };
    perch_ui::paint_rows(frame)
        .iter()
        .map(|run| run.text.clone())
        .collect()
}

/// Pumps the window until `done` holds, or [`TIMEOUT`] runs out.
fn pump_until(shell: &mut Shell, done: impl Fn(&Shell) -> bool) -> bool {
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
fn tree_of(shell: Shell) -> Value {
    let mut runtime = Runtime::new(shell, perch_ui::shell::view, WINDOW, Env::new(true, 2.0));
    runtime.frame(0.0);
    runtime.accessibility_json()
}

/// The node the app keyed `key`, wherever it sits.
fn find_key<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    if node.get("key").and_then(Value::as_str) == Some(key) {
        return Some(node);
    }
    let children = node.get("children").and_then(Value::as_array)?;
    children.iter().find_map(|child| find_key(child, key))
}

/// Whether `node`'s subtree says `needle` anywhere out loud.
fn says(node: &Value, needle: &str) -> bool {
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

#[test]
fn a_spawned_shells_output_reaches_a_frame_and_the_dock() {
    use_sh();
    const MARKER: &str = "dock-marker-3141";
    let worktree = tempfile::tempdir().unwrap();
    let mut shell = Shell::idle(vec![meta(worktree.path())], 0);
    shell
        .open_terminal()
        .expect("a shell spawns in the worktree");

    type_line(&mut shell, &format!("echo {MARKER}"));
    assert!(
        pump_until(&mut shell, |shell| frame_text(shell).contains(MARKER)),
        "the shell's output becomes a frame within {TIMEOUT:?}: {:?}",
        frame_text(&shell)
    );
    assert!(
        shell
            .active_terminal()
            .and_then(|tab| tab.frame())
            .is_some(),
        "the tab holds the frame its thread published"
    );

    let tree = tree_of(shell);
    let dock = find_key(&tree, "terminal").expect("the dock is in the tree");
    assert!(
        says(dock, MARKER),
        "the dock paints what the shell printed: {dock}"
    );
}

#[test]
fn the_dock_tells_the_shell_the_size_it_drew_at() {
    use_sh();
    let worktree = tempfile::tempdir().unwrap();
    let mut shell = Shell::idle(vec![meta(worktree.path())], 0);
    shell
        .open_terminal()
        .expect("a shell spawns in the worktree");
    assert_eq!(
        shell.active_terminal().map(|tab| tab.grid()),
        Some(perch_ui::shell::GRID),
        "a tab starts at the default grid"
    );

    // Laying the window out measures the dock; the next pump is what tells the pty. A layout read
    // is the previous frame's, so the first frame measures nothing and the second one does.
    let mut runtime = Runtime::new(shell, perch_ui::shell::view, WINDOW, Env::new(true, 2.0));
    runtime.frame(0.0);
    runtime.frame(0.0);
    runtime.state.pump(Duration::from_millis(5));

    let grid = runtime
        .state
        .active_terminal()
        .expect("the tab is open")
        .grid();
    assert!(
        grid.0 > perch_ui::shell::GRID.0 && grid.1 < perch_ui::shell::GRID.1,
        "the pty is told the dock's own shape, not the placeholder: {grid:?}"
    );
}

#[test]
fn a_shell_that_exits_says_so_instead_of_freezing_its_last_frame() {
    use_sh();
    const MARKER: &str = "still-here-2718";
    let worktree = tempfile::tempdir().unwrap();
    let mut shell = Shell::idle(vec![meta(worktree.path())], 0);
    shell
        .open_terminal()
        .expect("a shell spawns in the worktree");

    type_line(&mut shell, &format!("echo {MARKER}"));
    assert!(
        pump_until(&mut shell, |shell| frame_text(shell).contains(MARKER)),
        "the shell printed before it was told to exit: {:?}",
        frame_text(&shell)
    );

    type_line(&mut shell, "exit");
    assert!(
        pump_until(&mut shell, |shell| shell
            .active_terminal()
            .and_then(|tab| tab.ended())
            .is_some()),
        "the dock notices the shell exited within {TIMEOUT:?}"
    );

    let tree = tree_of(shell);
    let dock = find_key(&tree, "terminal").expect("the dock is in the tree");
    assert!(
        says(dock, "exited"),
        "the dock says the shell exited: {dock}"
    );
    assert!(
        !says(dock, MARKER),
        "the last frame is not painted as if the shell were still running: {dock}"
    );
}
