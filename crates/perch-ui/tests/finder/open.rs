//! What the rows do: Enter opens the file the tree would open, Escape leaves the pane under the
//! overlay alone, the scan skips what it must, and an empty list is said in words.

use std::path::{Path, PathBuf};

use perch_ui::shell::Pane;

use crate::harness::{
    chord, click, in_worktree, press, row_of, type_text, window, words, worktree,
};
use rune::Key;

#[test]
fn enter_opens_the_row_the_file_tree_would_open() {
    let dir = worktree(&["crates/perch-ui/src/shell/layout.rs", "README.md"]);

    let mut found = window(dir.path());
    let expected: PathBuf = in_worktree(&found, "crates/perch-ui/src/shell/layout.rs");
    chord(&mut found);
    type_text(&mut found, "layout");
    press(&mut found, Key::Enter);
    assert!(
        !found.state.finder_open(),
        "taking a row closes the overlay"
    );
    assert_eq!(found.state.pane(), &Pane::File(expected.clone()));
    assert_eq!(found.state.file_tabs(), [expected.clone()].as_slice());
    assert_eq!(
        found.state.files().unwrap().selected_path(),
        Some(expected.as_path()),
        "the panel read the file, as a tree press does"
    );

    let mut tree = window(dir.path());
    let row = row_of(&tree, &expected);
    click(&mut tree, &format!("file-row-{row}"));
    assert_eq!(
        tree.state.pane(),
        found.state.pane(),
        "the finder opens the same pane the tree opens"
    );
    assert_eq!(tree.state.file_tabs(), found.state.file_tabs());
}

#[test]
fn escape_closes_the_finder_and_leaves_the_previous_pane() {
    let dir = worktree(&["a.rs", "b.rs"]);
    let mut runtime = window(dir.path());
    let a = in_worktree(&runtime, "a.rs");
    let row = row_of(&runtime, &a);
    click(&mut runtime, &format!("file-row-{row}"));
    assert_eq!(runtime.state.pane(), &Pane::File(a.clone()));

    chord(&mut runtime);
    assert!(runtime.state.finder_open());
    press(&mut runtime, Key::Escape);
    assert!(!runtime.state.finder_open(), "Escape closes the overlay");
    assert_eq!(
        runtime.state.pane(),
        &Pane::File(a),
        "the pane under it is untouched"
    );
}

#[test]
fn the_scan_skips_git_target_and_node_modules() {
    let dir = worktree(&[
        ".git/config",
        "target/debug/out.o",
        "node_modules/x/index.js",
        "src/main.rs",
    ]);
    let mut runtime = window(dir.path());
    chord(&mut runtime);
    let files = runtime.state.finder_files();
    assert!(
        files.iter().any(|path| path == Path::new("src/main.rs")),
        "the source file is offered: {files:?}"
    );
    for gone in [".git", "target", "node_modules"] {
        assert!(
            !files
                .iter()
                .any(|path| path.components().any(|part| part.as_os_str() == gone)),
            "{gone} is never offered: {files:?}"
        );
    }
}

#[test]
fn the_list_is_read_when_the_overlay_opens_and_again_when_it_reopens() {
    let dir = worktree(&["src/one.rs"]);
    let mut runtime = window(dir.path());
    chord(&mut runtime);
    type_text(&mut runtime, "two");
    assert!(
        runtime.state.finder_rows().is_empty(),
        "two.rs was not there when the overlay opened"
    );

    // A file written while the overlay is shut is found by the next open, because the reopen reads
    // the worktree again rather than keeping the list the last open made.
    press(&mut runtime, Key::Escape);
    std::fs::write(dir.path().join("src/two.rs"), "// two\n").unwrap();
    chord(&mut runtime);
    type_text(&mut runtime, "two");
    assert_eq!(
        runtime.state.finder_rows().len(),
        1,
        "the reopen read the worktree again"
    );
}

#[test]
fn nothing_matching_and_an_empty_worktree_say_so_in_words() {
    let dir = worktree(&["src/main.rs"]);
    let mut runtime = window(dir.path());
    chord(&mut runtime);
    type_text(&mut runtime, "zzzz");
    assert!(runtime.state.finder_rows().is_empty());
    assert!(
        words(&runtime).contains("no file matches what is typed"),
        "a miss is said in words: {}",
        words(&runtime)
    );

    let empty = worktree(&[]);
    let mut runtime = window(empty.path());
    chord(&mut runtime);
    assert!(runtime.state.finder_rows().is_empty());
    assert!(
        words(&runtime).contains("this worktree has no files to find"),
        "an empty worktree is said in words: {}",
        words(&runtime)
    );
}
