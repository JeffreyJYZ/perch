//! A spawned shell: its output becomes a frame, and the pty is told the dock's own size.

use std::time::Duration;

use perch_ui::Shell;
use rune::{Env, Runtime};

use crate::harness::{
    TIMEOUT, WINDOW, find_key, frame_text, meta, pump_until, says, tree_of, type_line, use_sh,
};

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
