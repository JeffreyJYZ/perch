//! How the field ranks what it is given: a subsequence with a typo forgiven, the prefix first, and
//! no more rows than the cap.

use std::path::PathBuf;

use perch_ui::shell::MAX_ROWS;

use crate::harness::{chord, press, type_text, window, words, worktree};
use rune::Key;

#[test]
fn a_fuzzy_needle_finds_the_file_and_a_prefix_ranks_first() {
    let dir = worktree(&[
        "crates/perch-ui/src/shell/layout.rs",
        "docs/deep/nested/tree.rs",
        "tree/x.rs",
        "README.md",
    ]);
    let mut runtime = window(dir.path());

    // A subsequence with one dropped letter still reaches it: `shedlo` for `shell/layout`.
    chord(&mut runtime);
    type_text(&mut runtime, "shedlo");
    let rows = runtime.state.finder_rows();
    assert_eq!(
        rows.first().map(|row| row.path.clone()),
        Some(PathBuf::from("crates/perch-ui/src/shell/layout.rs")),
        "the fuzzy needle found the file: {rows:?}"
    );

    // A file whose own name starts with the needle comes before a shorter path that only holds
    // its letters, so `tree` reaches `tree.rs` and not `tree/x.rs` first.
    press(&mut runtime, Key::Escape);
    chord(&mut runtime);
    type_text(&mut runtime, "tree");
    let rows = runtime.state.finder_rows();
    assert_eq!(
        rows.first().map(|row| row.path.clone()),
        Some(PathBuf::from("docs/deep/nested/tree.rs")),
        "a name-prefix match ranks first: {rows:?}"
    );
}

#[test]
fn the_row_cap_is_honoured_and_the_overlay_says_the_total() {
    let files: Vec<String> = (0..30)
        .map(|index| format!("src/file{index:02}.rs"))
        .collect();
    let names: Vec<&str> = files.iter().map(String::as_str).collect();
    let dir = worktree(&names);
    let mut runtime = window(dir.path());
    chord(&mut runtime);
    type_text(&mut runtime, "file");

    assert_eq!(
        runtime.state.finder_rows().len(),
        MAX_ROWS,
        "only {MAX_ROWS} rows are drawn"
    );
    assert_eq!(runtime.state.finder_total(), 30, "every match is counted");
    assert!(
        words(&runtime).contains("30 matches"),
        "the overlay says how many matched: {}",
        words(&runtime)
    );
}
