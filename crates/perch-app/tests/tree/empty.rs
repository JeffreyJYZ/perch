//! A window that opened with no folder: every region draws, the column says why, and the one
//! thing it needs — a folder — is a button.

use perch_ui::Shell;

use crate::harness::{find_key, says, tree_of};

#[test]
fn a_window_with_no_folder_draws_every_region_and_offers_the_picker() {
    let reason = "/ is the filesystem root, not a project; pass --worktree <path> or open a folder";
    let tree = tree_of(Shell::idle(Vec::new(), 0).with_notice(reason));

    for region in ["sessions", "files", "transcript", "terminal", "left-column"] {
        assert!(
            find_key(&tree, region).is_some(),
            "the window with no folder draws no {region}:\n{tree}"
        );
    }

    let column = find_key(&tree, "left-column").expect("the sidebar is in the tree");
    assert!(
        says(column, "No folder open"),
        "the column says it has no folder: {column}"
    );
    assert!(
        says(column, reason),
        "the reason the launch directory was refused is in words, not an error dialog: {column}"
    );
    assert!(
        find_key(&tree, "open-folder").is_some(),
        "the column offers the folder picker: {column}"
    );
    assert!(
        find_key(&tree, "new-session").is_none(),
        "there is no worktree to start a session on before a folder is picked: {column}"
    );

    // Nothing spawned a shell: the dock is there, saying it has none.
    let dock = find_key(&tree, "terminal").expect("the dock is in the tree");
    assert!(
        says(dock, "no shell running"),
        "no pty was started for a window with no folder: {dock}"
    );
}
