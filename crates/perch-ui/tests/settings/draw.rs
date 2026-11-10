//! What the panel draws: every field's word, and a real box to press for each.

use perch_ui::Shell;

use crate::harness::layout;

#[test]
fn the_panel_draws_every_field_and_the_key_as_a_word() {
    let mut shell = Shell::idle(Vec::new(), 0);
    shell.toggle_settings();
    let runtime = layout(shell);
    let tree = runtime.accessibility_json().to_string();
    for word in [
        "Settings",
        "Base URL",
        "Model",
        "Theme",
        "Update check URL",
        "API key",
        "Save",
        "missing",
    ] {
        assert!(tree.contains(word), "the panel is missing {word}:\n{tree}");
    }
    // The words are laid out, not merely present: the panel has real boxes to press.
    let elements = runtime.elements();
    for key in [
        "settings-base-url",
        "settings-model",
        "settings-update-check",
        "settings-key",
        "settings-key-state",
        "settings-save",
    ] {
        let rect = elements
            .iter()
            .find(|element| element.key.as_deref() == Some(key))
            .map(|element| element.rect)
            .unwrap_or_else(|| panic!("the panel draws no {key}"));
        assert!(
            rect[2] > 0.0 && rect[3] > 0.0,
            "{key} has no box to press: {rect:?}"
        );
    }
}
