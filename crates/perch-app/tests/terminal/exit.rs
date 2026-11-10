//! A shell that exits says so, instead of freezing its last frame as if still printing.

use perch_ui::Shell;

use crate::harness::{
    TIMEOUT, find_key, frame_text, meta, pump_until, says, tree_of, type_line, use_sh,
};

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
