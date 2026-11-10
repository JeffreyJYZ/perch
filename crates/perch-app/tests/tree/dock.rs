//! The dock: its frames, its tabs, and where it sits under the transcript.

use perch_term::replay;
use perch_ui::Shell;

use crate::harness::{bounds, find_key, meta, says, tree_of};

#[test]
fn terminal_rows_paint_in_the_dock_and_never_in_the_sidebar() {
    let frame_text = "perch-dock-marker-3141";
    let frame = replay(format!("{frame_text}\r\n$ ").as_bytes(), 40, 3);
    let tree = tree_of(Shell::idle(vec![meta("work")], 0).with_frame("zsh", frame));

    let dock = find_key(&tree, "terminal").expect("the terminal dock is in the tree");
    assert!(
        says(dock, frame_text),
        "the dock paints the seeded frame's row: {dock}"
    );

    let sidebar = find_key(&tree, "left-column").expect("the sidebar is in the tree");
    assert!(
        !says(sidebar, frame_text),
        "terminal text belongs to the dock alone: {sidebar}"
    );
}

#[test]
fn each_terminal_tab_paints_only_its_own_rows() {
    let seeded = || {
        Shell::idle(vec![meta("work")], 0)
            .with_frame("first", replay(b"first-tab-marker", 40, 3))
            .with_frame("second", replay(b"second-tab-marker", 40, 3))
    };

    // A seeded tab is the newest, and the newest is the one the dock shows.
    let latest = tree_of(seeded());
    let dock = find_key(&latest, "terminal").expect("the dock is in the tree");
    assert!(
        find_key(dock, "terminal-tab-row").is_some(),
        "two tabs get a strip that names them: {dock}"
    );
    assert!(
        says(dock, "second-tab-marker"),
        "the dock paints the newest tab's frame: {dock}"
    );
    assert!(
        !says(dock, "first-tab-marker"),
        "the tab that is not shown keeps its rows out of the dock: {dock}"
    );

    // Selecting the other tab swaps which frame the dock paints.
    let mut shell = seeded();
    shell.select_terminal(0);
    let chosen = tree_of(shell);
    let dock = find_key(&chosen, "terminal").expect("the dock is in the tree");
    assert!(
        says(dock, "first-tab-marker"),
        "the selected tab's frame is painted: {dock}"
    );
    assert!(
        !says(dock, "second-tab-marker"),
        "the unselected tab's rows are gone: {dock}"
    );
}

#[test]
fn the_dock_sits_under_the_transcript_and_the_sidebar_runs_full_height() {
    let frame = replay(b"$ echo hi", 40, 3);
    let tree = tree_of(Shell::idle(vec![meta("work")], 0).with_frame("zsh", frame));

    let sidebar = bounds(find_key(&tree, "left-column").expect("the sidebar is in the tree"));
    let transcript = bounds(find_key(&tree, "transcript").expect("the transcript is in the tree"));
    let dock = bounds(find_key(&tree, "terminal").expect("the dock is in the tree"));
    let (window_w, window_h) = crate::harness::WINDOW;

    assert_eq!(sidebar[0], 0.0, "the sidebar starts at the window's left");
    assert_eq!(sidebar[1], 0.0, "the sidebar starts at the window's top");
    assert_eq!(
        sidebar[3],
        f64::from(window_h),
        "the sidebar runs the full window height"
    );

    assert!(
        dock[0] >= f64::from(perch_ui::COLUMN),
        "the dock starts right of the left column, at {}",
        dock[0]
    );
    assert!(
        dock[0] >= sidebar[0] + sidebar[2],
        "the dock does not overlap the sidebar"
    );
    assert!(
        transcript[1] < dock[1],
        "the dock is below the transcript ({} < {})",
        transcript[1],
        dock[1]
    );
    assert!(
        transcript[1] + transcript[3] <= dock[1] + 0.5,
        "the transcript ends where the dock starts"
    );
    assert!(
        dock[1] + dock[3] <= f64::from(window_h) + 0.5,
        "the dock stays inside the window"
    );
    assert_eq!(
        dock[2], transcript[2],
        "the dock is no wider than the transcript"
    );
    assert!(
        dock[0] + dock[2] <= f64::from(window_w) + 0.5,
        "the dock stays inside the window's width"
    );
}
