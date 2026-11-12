//! The settings path end to end, with the keychain kept out of it.
//!
//! Every app here is opened over a [`MemorySecrets`], so the tests prove what a save writes and
//! what the panel reads back without touching the login keychain. A keychain round trip needs a
//! signed-in human, so it is checked by hand once, not by the suite.

use std::sync::Arc;

use perch_app::settings::account_for;
use perch_app::{AppState, ProviderDefaults};
use perch_store::Settings;
use perch_store::secret::{MemorySecrets, SecretStore};
use perch_store::settings::{McpServerConfig, path_in};
use perch_ui::SettingsDraft;
use perch_ui::Shell;
use perch_ui::shell::KeyStatus;
use tempfile::TempDir;

/// A draft with every field stated, so a test names only the one it is about.
fn draft(base_url: &str, model: &str, theme: &str, api_key: &str) -> SettingsDraft {
    SettingsDraft {
        base_url: base_url.to_string(),
        model: model.to_string(),
        theme: theme.to_string(),
        api_key: api_key.to_string(),
        ..Default::default()
    }
}

/// An app over `data` that keeps its key in `secrets`.
fn open(data: &TempDir, secrets: &Arc<MemorySecrets>) -> AppState {
    AppState::open_with_secrets(
        data.path(),
        ProviderDefaults::default(),
        Box::new(Arc::clone(secrets)),
    )
    .expect("a data directory with no sessions opens")
}

#[test]
fn a_saved_key_reaches_the_store_and_never_the_settings_file() {
    let data = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecrets::new());
    let mut app = open(&data, &secrets);

    let status = app
        .save_settings(draft(
            "https://example.com/v1",
            "test/model",
            "light",
            "sk-test-secret",
        ))
        .expect("the memory store cannot refuse");

    assert_eq!(status, KeyStatus::Stored);
    let file = std::fs::read_to_string(path_in(data.path())).unwrap();
    assert!(file.contains("https://example.com/v1"), "{file}");
    assert!(file.contains("test/model"), "{file}");
    assert!(file.contains("light"), "{file}");
    assert!(
        !file.contains("sk-test-secret"),
        "the secret must never reach settings.toml:\n{file}"
    );
    // The trait is the one way in and out, so the key round-trips through it.
    assert_eq!(
        secrets.load(&account_for("default")).unwrap().as_deref(),
        Some("sk-test-secret")
    );
    assert_eq!(app.provider_defaults().api_key, "sk-test-secret");
}

#[test]
fn a_save_applies_without_a_restart_and_is_read_back_by_a_reopen() {
    let data = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecrets::new());
    let mut app = open(&data, &secrets);

    app.save_settings(draft(
        "https://example.com/v1",
        "test/model",
        "light",
        "sk-test-secret",
    ))
    .unwrap();

    // The running app's next session reads these, with nothing reopened and no restart.
    assert_eq!(app.provider_defaults().base_url, "https://example.com/v1");
    assert_eq!(app.provider_defaults().model, "test/model");
    assert_eq!(app.provider_defaults().api_key, "sk-test-secret");
    let worktree = tempfile::tempdir().unwrap();
    let index = app.new_session(worktree.path()).unwrap();
    assert_eq!(
        app.metas()[index].model,
        "test/model",
        "a session opened after the save asks for the new model"
    );

    let snapshot = app.settings();
    assert_eq!(snapshot.theme, "light");
    assert_eq!(snapshot.key, KeyStatus::Stored);

    // Opened again over the same directory, the panel still opens on what was saved.
    let reopened = open(&data, &secrets);
    let snapshot = reopened.settings();
    assert_eq!(snapshot.base_url, "https://example.com/v1");
    assert_eq!(snapshot.model, "test/model");
    assert_eq!(snapshot.key, KeyStatus::Stored);
}

#[test]
fn a_save_without_a_key_keeps_the_one_already_stored() {
    let data = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecrets::new());
    let mut app = open(&data, &secrets);

    app.save_settings(draft(
        "https://example.com/v1",
        "test/model",
        "dark",
        "sk-first",
    ))
    .unwrap();
    let status = app
        .save_settings(draft("https://example.com/v2", "other/model", "light", ""))
        .unwrap();

    assert_eq!(
        status,
        KeyStatus::Stored,
        "an empty key field does not clear the store"
    );
    assert_eq!(
        secrets.load(&account_for("default")).unwrap().as_deref(),
        Some("sk-first")
    );
    assert_eq!(app.provider_defaults().base_url, "https://example.com/v2");
    assert_eq!(app.provider_defaults().api_key, "sk-first");
}

#[test]
fn saving_keeps_the_settings_it_does_not_edit() {
    let data = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecrets::new());
    let mut app = open(&data, &secrets);

    // A server a person added by hand, written between the app opening and the save: a save of
    // the form's fields must leave it exactly where it is.
    let mut settings = Settings::load(data.path()).unwrap();
    settings.mcp_servers = vec![McpServerConfig::stdio("files", "/bin/cat", Vec::new())];
    settings.save(data.path()).unwrap();

    app.save_settings(draft("https://example.com/v1", "test/model", "dark", ""))
        .unwrap();

    let reloaded = Settings::load(data.path()).unwrap();
    assert_eq!(
        reloaded.mcp_servers.len(),
        1,
        "the servers a save does not edit are still there"
    );
    assert_eq!(reloaded.mcp_servers[0].name, "files");
}

#[test]
fn the_panels_save_reaches_the_app_and_the_store() {
    let data = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecrets::new());
    let mut shell = Shell::new(Vec::new(), 0).with_live(open(&data, &secrets));

    shell.toggle_settings();
    shell.stage_settings(draft(
        "https://example.com/v1",
        "test/model",
        "dark",
        "sk-from-the-panel",
    ));
    assert!(shell.save_settings(), "the panel saved through the app");

    assert!(
        shell.settings_open(),
        "the panel stays open, saying it saved"
    );
    assert_eq!(
        shell.settings_panel().and_then(|panel| panel.notice()),
        Some("saved")
    );
    assert_eq!(
        shell
            .settings_panel()
            .map(|panel| panel.key_words().to_string()),
        Some("stored".to_string())
    );
    assert_eq!(
        secrets.load(&account_for("default")).unwrap().as_deref(),
        Some("sk-from-the-panel")
    );
    let app = shell.live().expect("the window kept its app");
    assert_eq!(app.provider_defaults().model, "test/model");
}
