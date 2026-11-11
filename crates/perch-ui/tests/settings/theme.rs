//! The theme decides the kit: the saved word, a pick before Save, and the word a restart reads.
//!
//! Every assertion is on the colour the window was filled with, taken from the drawn element: the
//! harness lays every window out over a dark system, so a window that draws light proves the word
//! it saved decided the kit and the system did not.

use perch_ui::Shell;
use perch_ui::shell::{Live, Panels};
use rune::Color;
use rune::Runtime;
use rune_kit::Kit;

use crate::fake::Fake;
use crate::harness::{click, fill, layout, settle};

/// The background the window is drawn on, which is the kit's own deepest colour.
fn root<L: Live + Panels>(runtime: &Runtime<Shell<L>>) -> Color {
    fill(runtime, "perch")
}

#[test]
fn the_saved_theme_draws_the_window_and_the_system_does_not() {
    let dark = layout(Shell::new(Vec::new(), 0).with_live(Fake::default()));
    let light = layout(Shell::new(Vec::new(), 0).with_live(Fake {
        theme: "light".to_string(),
        ..Fake::default()
    }));
    assert_eq!(
        root(&dark),
        Kit::neutral(true).c.bg_deep,
        "a shell with no saved light word draws the dark kit"
    );
    assert_eq!(
        root(&light),
        Kit::neutral(false).c.bg_deep,
        "the saved theme wins over the dark system it was laid out under"
    );
    assert_ne!(
        root(&dark),
        root(&light),
        "the two themes draw different backgrounds"
    );

    // A window with no app behind it has no settings to read, so it draws the default.
    let idle = layout(Shell::idle(Vec::new(), 0));
    assert_eq!(root(&idle), Kit::neutral(true).c.bg_deep);
}

#[test]
fn a_pick_repaints_before_save_and_the_save_keeps_it() {
    let mut shell = Shell::new(Vec::new(), 0).with_live(Fake::default());
    shell.toggle_settings();
    let mut runtime = layout(shell);
    assert_eq!(root(&runtime), Kit::neutral(true).c.bg_deep);

    // Light is the second word the panel offers, and pressing it is the whole act.
    click(&mut runtime, "segment1");
    settle(&mut runtime);
    assert_eq!(
        runtime
            .state
            .settings_panel()
            .map(|panel| panel.theme().to_string()),
        Some("light".to_string()),
        "the panel holds the pick"
    );
    assert!(!runtime.state.dark(), "the window follows the pick");
    assert_eq!(
        root(&runtime),
        Kit::neutral(false).c.bg_deep,
        "the pick is drawn before Save"
    );
    assert!(
        runtime
            .state
            .live()
            .and_then(|fake| fake.saved.as_ref())
            .is_none(),
        "a pick writes nothing"
    );

    click(&mut runtime, "settings-save");
    settle(&mut runtime);
    let saved = runtime
        .state
        .live()
        .and_then(|fake| fake.saved.clone())
        .expect("the save reached the app");
    assert_eq!(saved.theme, "light", "the save wrote the picked theme");
    assert!(!runtime.state.dark(), "the save keeps the theme it wrote");
    assert_eq!(root(&runtime), Kit::neutral(false).c.bg_deep);

    // A restart: the app reads the same word back from settings.toml, and the window draws it.
    let restarted = layout(Shell::new(Vec::new(), 0).with_live(Fake {
        theme: saved.theme,
        ..Fake::default()
    }));
    assert_eq!(
        root(&restarted),
        Kit::neutral(false).c.bg_deep,
        "a restart draws the saved theme"
    );
}
