//! The model picker: the provider's list, the chosen one marked, and the typed field when the
//! list cannot be fetched.

use perch_ui::Shell;

use crate::fake::Fake;
use crate::harness::layout;

/// Three ids that do not occur in any panel label, so a tree search is about the list alone.
fn models() -> Vec<String> {
    vec![
        "vendor/portal".to_string(),
        "vendor/quasar".to_string(),
        "free/radiant".to_string(),
    ]
}

/// A laid-out window over `fake`, with the settings panel open.
///
/// The listing is taken by a pump after the pane opens, exactly as a frame takes it in the window.
fn settings_of(fake: Fake) -> rune::Runtime<Shell<Fake>> {
    let mut shell = Shell::new(Vec::new(), 0).with_live(fake);
    shell.toggle_settings();
    shell.pump(std::time::Duration::ZERO);
    // A panel opened over a chosen model starts folded so it cannot fill the pane; these tests
    // are about the list, so they unfold it the way the Show list press does.
    shell.toggle_models();
    layout(shell)
}

#[test]
fn the_settings_panel_lists_every_model_the_provider_returned() {
    let runtime = settings_of(Fake {
        models: models(),
        ..Fake::default()
    });
    let tree = runtime.accessibility_json().to_string();

    for id in models() {
        assert!(tree.contains(&id), "the picker is missing {id}:\n{tree}");
    }
    assert!(
        runtime.elements().iter().any(|element| {
            element.key.as_deref() == Some("settings-model") && element.rect[2] > 0.0
        }),
        "the model section has no box to press"
    );
}

#[test]
fn picking_a_model_in_the_panel_saves_it_without_a_second_press() {
    let mut runtime = settings_of(Fake {
        models: models(),
        ..Fake::default()
    });
    let row = runtime
        .elements()
        .into_iter()
        .find(|element| element.label.as_deref() == Some("vendor/quasar"))
        .expect("the list draws the row it was given");
    let at = (
        row.rect[0] + row.rect[2] * 0.5,
        row.rect[1] + row.rect[3] * 0.5,
    );
    runtime.pointer_down(at.0, at.1);
    runtime.pointer_up(at.0, at.1);
    runtime.frame(0.0);

    let saved = runtime
        .state
        .live()
        .and_then(|fake| fake.saved.clone())
        .expect("picking a row saved the draft");
    assert_eq!(
        saved.model, "vendor/quasar",
        "the picked model is the one saved"
    );
    assert_eq!(
        runtime
            .state
            .settings_panel()
            .map(|panel| panel.draft().model),
        Some("vendor/quasar".to_string())
    );
}

#[test]
fn a_list_that_cannot_be_fetched_is_worded_and_leaves_the_typed_field() {
    let runtime = settings_of(Fake {
        failure: Some("provider returned 404: no such endpoint".to_string()),
        ..Fake::default()
    });
    let tree = runtime.accessibility_json().to_string();

    assert!(
        tree.contains("models not listed"),
        "the panel says the list failed:\n{tree}"
    );
    assert!(
        tree.contains("404"),
        "the panel says why the list failed:\n{tree}"
    );
    assert!(
        runtime.elements().iter().any(|element| {
            element.role == "textbox" && element.label.as_deref() == Some("Model")
        }),
        "the typed model field is the fallback when no list can be drawn"
    );
}

#[test]
fn the_composer_shows_the_model_and_changing_it_there_picks_for_this_session() {
    let shell = Shell::new(Vec::new(), 0).with_live(Fake {
        model: "vendor/portal".to_string(),
        models: models(),
        ..Fake::default()
    });
    assert_eq!(shell.current_model(), "vendor/portal");

    let mut runtime = layout(shell);
    let tree = runtime.accessibility_json().to_string();
    assert!(
        tree.contains("vendor/portal"),
        "the composer shows the current model:\n{tree}"
    );

    // The control is a press, not a field: it opens the list in the transcript's place.
    let change = runtime
        .elements()
        .into_iter()
        .find(|element| element.key.as_deref() == Some("composer-model-change"))
        .expect("the composer draws its model control");
    let at = (
        change.rect[0] + change.rect[2] * 0.5,
        change.rect[1] + change.rect[3] * 0.5,
    );
    runtime.pointer_down(at.0, at.1);
    runtime.pointer_up(at.0, at.1);
    runtime.frame(0.0);
    // A press starts the listing and returns at once. The answer is taken by the window's own
    // poll, exactly as the open window takes it, and the frame that poll asks for is the one
    // that draws the list — so the test asks for that frame rather than poking the state.
    runtime.poll = Some(|shell: &mut Shell<Fake>, _: f64, _: &mut rune::Tx| {
        shell.pump(std::time::Duration::ZERO)
    });
    runtime.frame(0.0);
    assert!(runtime.state.settings_open(), "the model list opened");
    let tree = runtime.accessibility_json().to_string();
    for id in models() {
        assert!(tree.contains(&id), "the open list is missing {id}:\n{tree}");
    }

    // A row's press calls exactly this, with the row's id.
    assert!(
        runtime.state.pick_settings_model("free/radiant"),
        "the pick applied"
    );
    runtime.frame(0.0);

    let fake = runtime.state.live().expect("the window kept its app");
    assert_eq!(
        fake.picked.as_deref(),
        Some("free/radiant"),
        "the pick went to the session"
    );
    assert_eq!(runtime.state.current_model(), "free/radiant");
    assert!(!runtime.state.settings_open(), "the list closed on a pick");
    assert_eq!(fake.saved, None, "a session pick writes no settings file");
}
