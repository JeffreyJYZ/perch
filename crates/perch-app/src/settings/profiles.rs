//! The provider profiles as the app keeps them: a key per profile, and the one that is active.
//!
//! A key is filed under the profile's name, so two providers keep two keys and neither can sign
//! the other's requests. The key a file written before the list existed keeps is filed under the
//! old account, and the profile it became inherits it: the file changed shape, not the secret
//! beside it.

use perch_store::Settings;
use perch_store::secret::SecretStore;
use perch_ui::shell::{KeyStatus, ProviderView};

use super::{KEY_ACCOUNT, KEY_VARS};

/// The account a profile's key is filed under.
pub fn account_for(name: &str) -> String {
    format!("{KEY_ACCOUNT}:{name}")
}

/// The key of the profile named `name`, when one is stored for it.
///
/// The profile a file written before the list existed became keeps its key under the old
/// account, so that key is read too.
pub fn stored_key(secrets: &dyn SecretStore, name: &str) -> Option<String> {
    if let Some(key) = own_key(secrets, &account_for(name)) {
        return Some(key);
    }
    if name == perch_store::settings::DEFAULT_PROFILE {
        return own_key(secrets, KEY_ACCOUNT);
    }
    None
}

/// The key a turn is signed with: the profile's own, then the one the shell exported.
///
/// The environment matters because a key exported in a shell is the way a developer already has
/// one; asking them to paste it into a settings panel, and then failing at it, is the behaviour
/// that made this app unusable.
pub fn runtime_key(secrets: &dyn SecretStore, name: &str) -> Option<String> {
    stored_key(secrets, name).or_else(environment_key)
}

/// Whether a key is stored for the profile named `name`, as the panel says it.
pub fn key_status(secrets: &dyn SecretStore, name: &str) -> KeyStatus {
    match stored_key(secrets, name) {
        Some(_) => KeyStatus::Stored,
        None => KeyStatus::Missing,
    }
}

/// The key stored under one account, and nothing else.
fn own_key(secrets: &dyn SecretStore, account: &str) -> Option<String> {
    let key = secrets.load(account).ok().flatten()?;
    let key = key.trim().to_string();
    (!key.is_empty()).then_some(key)
}

/// Copies the key an older file kept under the old account into the profile it became.
///
/// The key is never lost by the migration: the file changed shape, and the secret beside it is
/// filed under the profile's own name from then on.
pub fn adopt_legacy_key(settings: &Settings, secrets: &dyn SecretStore) {
    let Some(profile) = settings.profile(perch_store::settings::DEFAULT_PROFILE) else {
        return;
    };
    if own_key(secrets, &account_for(&profile.name)).is_some() {
        return;
    }
    let Some(key) = own_key(secrets, KEY_ACCOUNT) else {
        return;
    };
    let _ = secrets.store(&account_for(&profile.name), &key);
}

/// The profiles the settings file holds, as the settings panel draws them.
pub fn views(settings: &Settings, secrets: &dyn SecretStore) -> Vec<ProviderView> {
    settings
        .providers
        .iter()
        .map(|profile| ProviderView {
            name: profile.name.clone(),
            base_url: profile.base_url.clone(),
            model: profile.model.clone(),
            key: key_status(secrets, &profile.name),
            active: profile.name == settings.active_provider,
        })
        .collect()
}

/// The first of `names` that is set to something, trimmed.
pub fn first_env(names: &[&str]) -> Option<String> {
    names
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
}

/// The key the shell exported, when it exported one.
fn environment_key() -> Option<String> {
    first_env(&KEY_VARS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use perch_store::secret::MemorySecrets;

    #[test]
    fn two_profiles_keep_two_keys() {
        let secrets = MemorySecrets::new();
        secrets.store(&account_for("one"), "sk-one").unwrap();
        secrets.store(&account_for("two"), "sk-two").unwrap();

        assert_eq!(stored_key(&secrets, "one").as_deref(), Some("sk-one"));
        assert_eq!(stored_key(&secrets, "two").as_deref(), Some("sk-two"));
        assert_eq!(key_status(&secrets, "one"), KeyStatus::Stored);
        assert_eq!(key_status(&secrets, "three"), KeyStatus::Missing);
    }

    #[test]
    fn the_key_an_old_file_kept_is_inherited_by_the_profile_it_became() {
        let secrets = MemorySecrets::new();
        secrets.store(KEY_ACCOUNT, "sk-old").unwrap();

        assert_eq!(
            stored_key(&secrets, perch_store::settings::DEFAULT_PROFILE).as_deref(),
            Some("sk-old"),
            "the profile the old file became keeps the key that was saved"
        );
        assert_eq!(
            stored_key(&secrets, "second"),
            None,
            "and no other profile does"
        );
    }
}
