//! Saving: the key goes to the app, the panel only ever says the word.

use perch_ui::Shell;
use perch_ui::shell::KeyStatus;

use crate::fake::Fake;
use crate::harness::{draft, layout};

#[test]
fn a_saved_key_is_worded_and_never_held_by_the_panel() {
    let mut shell = Shell::new(Vec::new(), 0).with_live(Fake {
        key: KeyStatus::Missing,
        saved: None,
    });
    shell.toggle_settings();
    let mut draft = draft("https://example.com/v1", "test/model", "dark", "sk-secret");
    draft.update_check = "https://release.example.com/latest.json".to_string();
    shell.stage_settings(draft);

    assert!(shell.save_settings(), "the app took the draft");
    let panel = shell.settings_panel().expect("the panel stays open");
    assert_eq!(panel.key_words(), "stored", "the panel says the word");
    assert_eq!(
        panel.draft().api_key,
        "",
        "the key field is emptied: the panel never shows the secret again"
    );
    assert_eq!(panel.notice(), Some("saved"));

    let saved = shell
        .live()
        .and_then(|fake| fake.saved.clone())
        .expect("the app was handed the key");
    assert_eq!(
        saved.api_key, "sk-secret",
        "the key went to the app, not the panel"
    );
    assert_eq!(
        saved.update_check, "https://release.example.com/latest.json",
        "the update check is saved with the other fields"
    );

    let tree = layout(shell).accessibility_json().to_string();
    assert!(tree.contains("stored"), "the panel shows the word: {tree}");
    assert!(
        !tree.contains("sk-secret"),
        "the secret is never drawn:\n{tree}"
    );
}
