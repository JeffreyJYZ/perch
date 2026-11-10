//! A press on a control a panel declares, all the way through the window.
//!
//! A button and an input are drawn by the host from a plugin's model, so the proof they are wired
//! is the model moving: the counter's own counter goes up when its button is pressed, and the text
//! typed into its field is what the plugin sees. The last test is the press that cannot be handed
//! over at all — it is said in the column rather than swallowed.

mod support;

use support::{App, click, dir_with, enter, tree, type_text, window};

#[test]
fn pressing_a_panel_button_changes_the_line_the_column_draws() {
    let dir = dir_with(&[("counter.wasm", "counter.wasm")]);
    let mut runtime = window(App::over(dir.path()));
    assert!(
        tree(&runtime).contains("count: 0"),
        "the fixture's first panel is not drawn: {}",
        tree(&runtime)
    );

    // The press is the button's own, so the drawn panel must declare it for the click to land.
    click(&mut runtime, "panel-button-bump");

    let drawn = tree(&runtime);
    assert!(
        drawn.contains("count: 1"),
        "the button press did not reach the plugin and back: {drawn}"
    );
    assert!(
        !drawn.contains("count: 0"),
        "the column is drawing a model it re-read before the press: {drawn}"
    );

    // A second press counts again: the model is re-read on every frame, not once after the first.
    click(&mut runtime, "panel-button-bump");
    assert!(tree(&runtime).contains("count: 2"), "{}", tree(&runtime));

    // And a different control of the same panel answers too.
    click(&mut runtime, "panel-button-reset");
    assert!(tree(&runtime).contains("count: 0"), "{}", tree(&runtime));
}

#[test]
fn submitting_a_panel_input_reports_the_text_the_field_holds() {
    let dir = dir_with(&[("counter.wasm", "counter.wasm")]);
    let mut runtime = window(App::over(dir.path()));

    // A click focuses the field; a click alone submits nothing.
    click(&mut runtime, "panel-input-field-note");
    assert!(
        !tree(&runtime).contains("note:"),
        "an unfocused, untyped field reported a note: {}",
        tree(&runtime)
    );

    type_text(&mut runtime, "hi");
    assert!(
        !tree(&runtime).contains("note: hi"),
        "the field reported its text before it was submitted: {}",
        tree(&runtime)
    );

    enter(&mut runtime);
    let drawn = tree(&runtime);
    assert!(
        drawn.contains("note: hi"),
        "the submit did not carry the text the field held: {drawn}"
    );
    // The value reached the plugin as the input's own, not as the panel's title or its counter.
    assert!(drawn.contains("count: 0"), "{drawn}");
}

#[test]
fn a_press_the_app_refuses_is_said_in_the_column() {
    let dir = dir_with(&[("counter.wasm", "counter.wasm")]);
    let refusal = "plugin `counter` declares no control `bump`, so nothing was reported";
    let mut runtime = window(App::refusing(dir.path(), refusal));

    click(&mut runtime, "panel-button-bump");

    let drawn = tree(&runtime);
    assert!(
        drawn.contains(refusal),
        "a refused press left no trace in the column: {drawn}"
    );
    // A refused press is not a press that worked: the model did not move.
    assert!(drawn.contains("count: 0"), "{drawn}");
}
