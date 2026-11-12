//! The switch: one action, and the next turn asks the provider that is active.

use std::sync::Arc;

use perch_app::settings::account_for;
use perch_store::Settings;
use perch_store::secret::{MemorySecrets, SecretStore};

use crate::{activate_draft, draft, open, profile_draft};

/// Two providers on file, the first one active.
fn two_providers() -> (tempfile::TempDir, Arc<MemorySecrets>, perch_app::AppState) {
    let data = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecrets::new());
    let mut app = open(&data, &secrets);
    app.save_settings(draft("https://one.test/v1", "one/model", "sk-one"))
        .unwrap();
    app.save_settings(profile_draft(
        "second",
        "https://two.test/v1",
        "two/model",
        "sk-two",
    ))
    .unwrap();
    (data, secrets, app)
}

#[test]
fn a_second_profile_is_added_and_keeps_its_own_key() {
    let (data, secrets, app) = two_providers();

    let settings = Settings::load(data.path()).unwrap();
    assert_eq!(settings.providers.len(), 2, "the list holds both providers");
    assert_eq!(
        settings.active_provider, "default",
        "adding a provider does not switch the app to it"
    );
    assert_eq!(
        app.provider_defaults().base_url,
        "https://one.test/v1",
        "the active provider is still the one the app asks"
    );
    assert_eq!(
        secrets.load(&account_for("default")).unwrap().as_deref(),
        Some("sk-one")
    );
    assert_eq!(
        secrets.load(&account_for("second")).unwrap().as_deref(),
        Some("sk-two"),
        "each provider keeps its own key"
    );
}

#[test]
fn activating_a_provider_takes_effect_for_the_next_turn() {
    let (data, _secrets, mut app) = two_providers();

    // One action: the save that names the provider to use.
    app.save_settings(activate_draft("second", "https://two.test/v1", "two/model"))
        .unwrap();

    let settings = Settings::load(data.path()).unwrap();
    assert_eq!(
        settings.active_provider, "second",
        "the file says which is active"
    );
    assert_eq!(
        app.provider_defaults().base_url,
        "https://two.test/v1",
        "and the running app asks it, with no restart"
    );
    assert_eq!(app.provider_defaults().model, "two/model");
    assert_eq!(app.provider_defaults().api_key, "sk-two");

    let worktree = tempfile::tempdir().unwrap();
    let index = app.new_session(worktree.path()).unwrap();
    assert_eq!(
        app.metas()[index].model,
        "two/model",
        "a session opened after the switch asks the provider that is active"
    );
}

#[test]
fn the_app_switches_provider_without_a_restart_too() {
    let (data, _secrets, mut app) = two_providers();

    app.activate_profile("second").unwrap();

    assert_eq!(app.provider_defaults().base_url, "https://two.test/v1");
    assert_eq!(app.provider_defaults().api_key, "sk-two");
    assert_eq!(
        Settings::load(data.path()).unwrap().active_provider,
        "second",
        "the switch is written, so the next window opens on it"
    );
    assert!(
        app.activate_profile("gone").is_err(),
        "a name no provider carries is refused in words"
    );
}

#[test]
fn the_settings_panel_lists_the_providers_and_the_active_one() {
    let (_data, _secrets, app) = two_providers();

    let snapshot = app.settings();
    assert_eq!(snapshot.profiles.len(), 2, "the panel lists both providers");
    assert_eq!(snapshot.profiles[0].name, "default");
    assert!(snapshot.profiles[0].active, "and says which one is active");
    assert_eq!(snapshot.profiles[1].name, "second");
    assert!(!snapshot.profiles[1].active);
    assert_eq!(
        snapshot.profiles[1].key,
        perch_ui::shell::KeyStatus::Stored,
        "each profile says whether it has a key, never what it is"
    );
}
