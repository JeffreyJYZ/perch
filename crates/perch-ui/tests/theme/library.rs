//! The library: what it lists, what it refuses, and what a saved name that is not there draws.
//!
//! A theme that does not appear and gives no reason is the worst outcome, so every one of these
//! tests is about a name being present with words attached.

use perch_ui::kit;
use perch_ui::theme::{Appearance, Library};
use tempfile::TempDir;

/// A themes directory with one good file and one that will not parse.
fn themes() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("dusk.toml"),
        "bg = \"#101014\"\nink = \"#f0e8d8\"\nsignal = \"#d98a2b\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("broken.toml"),
        "bg = \"#101014\"\nink = 3\n",
    )
    .unwrap();
    dir
}

#[test]
fn the_two_builtins_are_always_there_and_the_files_come_with_them() {
    let empty = tempfile::tempdir().unwrap();
    let none = Library::at(empty.path());
    assert_eq!(none.names(), "dark, light");
    assert_eq!(none.themes().len(), 2);

    let dir = themes();
    let library = Library::at(dir.path());
    assert_eq!(library.names(), "dark, light, broken, dusk");
    assert!(library.choose("dusk").is_ok());
}

#[test]
fn a_file_that_will_not_parse_is_listed_with_its_error() {
    let dir = themes();
    let library = Library::at(dir.path());
    let theme = library
        .get("broken")
        .expect("a broken file is still listed");
    let error = theme.error().expect("and it says why").to_string();

    assert!(!error.is_empty());
    assert!(
        error.contains("ink") || error.contains("parse") || error.contains("integer"),
        "the error names the field or the parse: {error}"
    );
    assert!(
        library.choose("broken").is_err(),
        "an unreadable theme is not choosable"
    );
    let listed = library.listing("dark").join("\n");
    assert!(listed.contains("broken"), "{listed}");
    assert!(listed.contains("will not load"), "{listed}");
}

#[test]
fn an_unknown_name_falls_back_to_dark_and_the_pane_says_so() {
    let dir = themes();
    let library = Library::at(dir.path());
    let appearance = Appearance::resolve(&library, "duskish", None);

    assert_eq!(appearance.theme, "dark");
    assert_eq!(appearance.anchors, kit::builtin(true));
    let note = appearance
        .note
        .expect("the pane says why it is not the saved one");
    assert!(note.contains("duskish"), "{note}");
    assert!(note.contains("drawing dark"), "{note}");
}

#[test]
fn a_saved_name_that_does_load_switches_the_kit() {
    let dir = themes();
    let library = Library::at(dir.path());
    let appearance = Appearance::resolve(&library, "dusk", None);

    assert_eq!(appearance.theme, "dusk");
    assert_eq!(appearance.anchors.bg, rune::color::hex("#101014").unwrap());
    assert_ne!(appearance.anchors, kit::builtin(true));
    assert!(appearance.note.is_none());
    assert_eq!(appearance.kit().c.bg_base, appearance.anchors.bg);
}

#[test]
fn theme_switches_to_a_listed_name_and_refuses_one_that_is_not() {
    let dir = themes();
    let library = Library::at(dir.path());

    assert_eq!(
        library.choose("dusk").unwrap().anchors().unwrap().bg,
        rune::color::hex("#101014").unwrap()
    );

    let refused = library
        .choose("duskish")
        .expect_err("an unknown name is refused");
    assert!(refused.contains("duskish"), "{refused}");
    assert!(
        refused.contains("dark, light"),
        "it names what exists: {refused}"
    );
    assert!(refused.contains("/themes"), "and where to look: {refused}");
}

#[test]
fn themes_lists_every_name_and_marks_the_active_one() {
    let dir = themes();
    let library = Library::at(dir.path());
    let listed = library.listing("light");

    assert_eq!(listed[0], "dark");
    assert!(listed.contains(&"light (active)".to_string()), "{listed:?}");
    assert!(listed.contains(&"dusk".to_string()), "{listed:?}");
}
