//! The migration: a file written before the provider list existed, and the key beside it.

use std::sync::Arc;

use perch_app::settings::{KEY_ACCOUNT, account_for};
use perch_store::Settings;
use perch_store::secret::{MemorySecrets, SecretStore};

use crate::open_reading_the_file;

#[test]
fn an_old_settings_file_keeps_its_provider_and_its_key() {
    let data = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecrets::new());
    // The key an older build saved, under the account it used.
    secrets.store(KEY_ACCOUNT, "sk-old").unwrap();
    std::fs::write(
        perch_store::settings::path_in(data.path()),
        "provider_base_url = \"https://old.test/v1\"\nmodel = \"old/model\"\n",
    )
    .unwrap();

    let app = open_reading_the_file(&data, &secrets);

    assert_eq!(
        app.provider_defaults().base_url,
        "https://old.test/v1",
        "the migration asks the provider the old file named"
    );
    assert_eq!(app.provider_defaults().model, "old/model");
    assert_eq!(
        app.provider_defaults().api_key,
        "sk-old",
        "the migration never loses the key that was saved"
    );

    let settings = Settings::load(data.path()).unwrap();
    assert_eq!(settings.providers.len(), 1, "the old provider is one entry");
    assert_eq!(settings.active_provider, "default");
    assert_eq!(
        secrets.load(&account_for("default")).unwrap().as_deref(),
        Some("sk-old"),
        "and the key is filed under the profile it became"
    );
}
