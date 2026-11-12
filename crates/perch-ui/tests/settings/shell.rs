//! The shell a new dock tab runs: empty is this machine's own, a command nothing can run is
//! refused in words, and the field is drawn with the line that was saved.

use perch_ui::Shell;

use crate::fake::Fake;
use crate::harness::{draft, layout};

/// A draft that states every field a save checks, so only the shell varies.
fn shell_draft(shell: &str) -> perch_ui::SettingsDraft {
    let mut draft = draft("https://example.com/v1", "test/model", "dark", "");
    draft.shell = shell.to_string();
    draft
}

#[test]
fn an_empty_shell_is_the_default_and_not_an_error() {
    for blank in ["", "   "] {
        shell_draft(blank)
            .validate()
            .expect("an empty shell means this machine's own, so nothing is refused");
    }

    let mut shell = Shell::new(Vec::new(), 0).with_live(Fake::default());
    shell.toggle_settings();
    shell.stage_settings(shell_draft(""));
    assert!(shell.save_settings(), "the app took the empty field");
    assert_eq!(
        shell
            .live()
            .and_then(|fake| fake.saved.as_ref().map(|saved| saved.shell.clone())),
        Some(String::new()),
        "an empty field is saved as empty, which is the default"
    );
}

#[test]
fn a_shell_this_machine_cannot_run_is_refused_in_words() {
    let refused = shell_draft("perch-no-such-shell")
        .validate()
        .expect_err("a command nothing can run is refused");
    assert_eq!(refused.field, perch_ui::Field::Shell);
    assert!(
        refused.words.contains("shell"),
        "the refusal names the field: {}",
        refused.words
    );
    assert!(
        refused.words.contains("perch-no-such-shell"),
        "the refusal names the command: {}",
        refused.words
    );

    // The panel is where the refusal lands: it stays open, says which field, and nothing is saved.
    let mut shell = Shell::new(Vec::new(), 0).with_live(Fake::default());
    shell.toggle_settings();
    shell.stage_settings(shell_draft("perch-no-such-shell"));
    assert!(!shell.save_settings(), "the panel refused the save");
    assert!(shell.settings_open(), "the panel stays open on a refusal");
    assert_eq!(
        shell
            .settings_panel()
            .and_then(|panel| panel.error())
            .map(|error| error.field),
        Some(perch_ui::Field::Shell),
        "the shell field is the one named"
    );
    assert!(
        shell.live().is_some_and(|fake| fake.saved.is_none()),
        "a refused field never reaches the app"
    );

    // A command this machine does have is what the field is for.
    shell_draft("/bin/sh -l")
        .validate()
        .expect("/bin/sh is a command this machine runs");
}

#[test]
fn the_panel_draws_the_saved_line_beside_the_font_size() {
    let mut shell = Shell::new(Vec::new(), 0).with_live(Fake {
        shell: "/bin/sh -l".to_string(),
        ..Fake::default()
    });
    shell.toggle_settings();
    assert_eq!(
        shell.settings_panel().map(|panel| panel.draft().shell),
        Some("/bin/sh -l".to_string()),
        "the field opens on the saved line"
    );

    let runtime = layout(shell);
    let elements = runtime.elements();
    let field = elements
        .iter()
        .find(|element| element.key.as_deref() == Some("settings-shell-command"))
        .unwrap_or_else(|| panic!("the panel draws no shell field"));
    assert!(
        field.rect[2] > 0.0 && field.rect[3] > 0.0,
        "the shell field has a box to type in: {:?}",
        field.rect
    );
    let tree = runtime.accessibility_json().to_string();
    assert!(
        tree.contains("/bin/sh -l"),
        "the saved line is what the field shows:\n{tree}"
    );
}
