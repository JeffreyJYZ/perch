//! The providers section of the settings panel: the list, the press that selects one, the press
//! that adds one, and the press that makes one active.
//!
//! No key is ever read back into a field: the panel shows the word, and only ever takes a new
//! key.

use perch_ui::Shell;
use perch_ui::shell::{KeyStatus, ProviderView};

use crate::fake::Fake;
use crate::harness::layout;

/// A fake whose settings carry two providers, the first one active.
fn two_providers() -> Fake {
    Fake {
        key: KeyStatus::Stored,
        profiles: vec![
            ProviderView {
                name: "default".to_string(),
                base_url: "https://one.test/v1".to_string(),
                model: "one/model".to_string(),
                key: KeyStatus::Stored,
                active: true,
            },
            ProviderView {
                name: "second".to_string(),
                base_url: "https://two.test/v1".to_string(),
                model: "two/model".to_string(),
                key: KeyStatus::Missing,
                active: false,
            },
        ],
        ..Fake::default()
    }
}

/// A window with the settings panel open over `fake`.
fn open(fake: Fake) -> rune::Runtime<Shell<Fake>> {
    let mut shell = Shell::new(Vec::new(), 0).with_live(fake);
    shell.toggle_settings();
    layout(shell)
}

/// Clicks the element with `key`, the way a person would.
fn press(runtime: &mut rune::Runtime<Shell<Fake>>, key: &str) {
    let at = runtime
        .elements()
        .into_iter()
        .find(|element| element.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("the panel draws no {key}"))
        .rect;
    let (x, y) = (at[0] + at[2] * 0.5, at[1] + at[3] * 0.5);
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    runtime.frame(0.0);
}

#[test]
fn the_panel_lists_the_providers_and_marks_the_active_one() {
    let runtime = open(two_providers());
    let tree = runtime.accessibility_json().to_string();

    assert!(
        tree.contains("PROVIDERS"),
        "the list has its own section:\n{tree}"
    );
    assert!(tree.contains("default"), "{tree}");
    assert!(tree.contains("second"), "{tree}");
    assert!(tree.contains("(active)"), "{tree}");
    assert!(tree.contains("two/model"), "{tree}");
}

#[test]
fn the_fields_open_on_the_active_provider() {
    let runtime = open(two_providers());
    let panel = runtime.state.settings_panel().expect("the panel is open");

    assert_eq!(panel.draft().base_url, "https://one.test/v1");
    assert_eq!(panel.draft().model, "one/model");
    assert_eq!(panel.draft().profile, "default");
}

#[test]
fn selecting_a_provider_fills_the_fields_and_never_reads_a_key_back() {
    let mut runtime = open(two_providers());

    press(&mut runtime, "settings-profile-1");

    let panel = runtime.state.settings_panel().expect("the panel is open");
    assert_eq!(panel.draft().base_url, "https://two.test/v1");
    assert_eq!(panel.draft().model, "two/model");
    assert_eq!(panel.draft().profile, "second");
    assert_eq!(
        panel.draft().api_key,
        "",
        "a key is write-only: the panel never reads one back into a field"
    );
    assert_eq!(
        panel.key_words(),
        "missing",
        "the word says the state, not the secret"
    );
}

#[test]
fn adding_a_provider_adds_a_row_and_selects_it() {
    let mut runtime = open(two_providers());

    press(&mut runtime, "settings-provider-add");

    let panel = runtime.state.settings_panel().expect("the panel is open");
    assert_eq!(panel.profiles().len(), 3, "the new provider is in the list");
    assert_eq!(
        panel.draft().profile,
        "provider-3",
        "and it is the one the fields belong to"
    );
    assert_eq!(
        panel.draft().base_url,
        "",
        "its fields are empty until they are filled in"
    );
}

#[test]
fn the_active_press_saves_the_draft_and_names_the_provider_to_use() {
    let mut runtime = open(two_providers());

    press(&mut runtime, "settings-profile-1");
    press(&mut runtime, "settings-provider-activate");

    let saved = runtime
        .state
        .live()
        .and_then(|fake| fake.saved.clone())
        .expect("the press handed the app a draft");
    assert_eq!(
        saved.profile, "second",
        "the save names the provider it edits"
    );
    assert!(saved.activate, "and asks for it to become the active one");
    assert_eq!(saved.base_url, "https://two.test/v1");
    assert_eq!(saved.model, "two/model");
}

#[test]
fn a_refused_field_stops_the_save_before_the_provider_is_used() {
    let mut runtime = open(two_providers());

    press(&mut runtime, "settings-profile-1");
    // An empty model is refused, and the pane says which field refused it.
    runtime
        .state
        .stage_settings(crate::harness::draft("https://two.test/v1", "", "dark", ""));
    press(&mut runtime, "settings-provider-activate");

    let panel = runtime.state.settings_panel().expect("the panel is open");
    assert_eq!(
        panel.error().map(|invalid| invalid.words.clone()),
        Some("model: pick one from the provider's list".to_string()),
        "the refusal names the field, and nothing was saved"
    );
    assert!(
        runtime
            .state
            .live()
            .and_then(|fake| fake.saved.clone())
            .is_none(),
        "a refused draft is never handed to the app"
    );
}
