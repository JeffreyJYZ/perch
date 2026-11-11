//! The file finder: ⌘P opens one field over the worktree's files, typing narrows them by fuzzy
//! subsequence, and Enter opens the row a tree press would open.
//!
//! Every window here is laid out through Rune with the binary's own key map, so a press reaches
//! the field only if the field is really focused on the frame it is mounted.

mod harness;
mod open;
mod rank;

use harness::{chord, type_text, window, worktree};

/// The chord, then the very next keystroke: the overlay's field takes the keyboard on the frame it
/// is mounted, so nothing has to be clicked before typing.
#[test]
fn the_chord_opens_the_finder_and_the_next_key_reaches_the_field() {
    let dir = worktree(&["crates/perch-ui/src/shell/layout.rs", "README.md"]);
    let mut runtime = window(dir.path());
    chord(&mut runtime);
    assert!(runtime.state.finder_open(), "⌘P opened the overlay");

    type_text(&mut runtime, "layout");
    assert_eq!(runtime.state.finder_query_text(), "layout");
    assert_eq!(
        runtime.state.finder_rows().len(),
        1,
        "the letters narrowed the list: {:?}",
        runtime.state.finder_rows()
    );
    assert_eq!(
        runtime.state.prompt_text(),
        "",
        "the composer received none of the keys"
    );

    // A second chord closes it, so the same press opens and shuts the overlay.
    chord(&mut runtime);
    assert!(!runtime.state.finder_open());
}
