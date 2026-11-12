//! A theme names three anchors, in either notation, and every other tone is derived from them.

use perch_ui::kit;
use perch_ui::theme::Library;
use rune::{color::hex, oklch};
use tempfile::TempDir;

/// A theme written the two ways a colour may be: OKLCH where the direction doc states one, and a
/// hex a person copied from a design tool.
const DUSK: &str = r##"
bg = "oklch(0.21 0.02 40)"
ink = "#f2ead8"
signal = "#d98a2b"
ok = "oklch(0.7 0.12 150)"
"##;

fn themes(file: &str, text: &str) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(file), text).unwrap();
    dir
}

#[test]
fn three_anchors_parse_from_oklch_and_from_hex() {
    let dir = themes("dusk.toml", DUSK);
    let library = Library::at(dir.path());
    let theme = library.choose("dusk").expect("the file parses");
    let anchors = theme.anchors().expect("a parsed theme has anchors");

    assert_eq!(anchors.bg, oklch(0.21, 0.02, 40.0));
    assert_eq!(anchors.ink, hex("#f2ead8").unwrap());
    assert_eq!(anchors.signal, hex("#d98a2b").unwrap());
    assert_eq!(anchors.ok, Some(oklch(0.7, 0.12, 150.0)));
    assert_eq!(
        anchors.fail, None,
        "a state the file did not name stays derived"
    );
}

#[test]
fn the_derived_tones_follow_the_anchors() {
    let dir = themes("dusk.toml", DUSK);
    let library = Library::at(dir.path());
    let anchors = library
        .choose("dusk")
        .expect("the file parses")
        .anchors()
        .expect("a parsed theme has anchors");
    let dusk = kit::from_anchors(&anchors, None);
    let dark = kit::perch(true);

    assert_eq!(dusk.c.bg_base, anchors.bg);
    assert_ne!(
        dusk.c.bg_deep, dark.c.bg_deep,
        "a file theme's surfaces are derived from its own canvas, not the built-in one"
    );
    assert_ne!(dusk.c.text_accent, dark.c.text_accent);
    assert!(dusk.dark, "cream ink on a dark canvas is the dark kit");
}

#[test]
fn a_light_theme_is_light_because_its_ink_is_darker_than_its_canvas() {
    let dir = themes(
        "paper.toml",
        "bg = \"#f6f1e6\"\nink = \"#20201e\"\nsignal = \"#a35a12\"\n",
    );
    let library = Library::at(dir.path());
    let anchors = library
        .choose("paper")
        .expect("the file parses")
        .anchors()
        .expect("a parsed theme has anchors");

    assert!(!anchors.dark());
    assert!(!kit::from_anchors(&anchors, None).dark);
}
