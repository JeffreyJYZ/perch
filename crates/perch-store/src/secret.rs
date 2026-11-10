//! Provider secrets live in the keychain, never in TOML, a session file or a log line.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::Result;

/// The keychain service every perch secret is filed under.
pub const SERVICE: &str = "dev.jyz.perch";

/// Where a provider key is kept.
///
/// Everything that reads or writes a key goes through this trait, so a test hands over
/// [`MemorySecrets`] and proves the whole settings path without touching the login keychain.
/// The one store the app ships is [`Keychain`].
pub trait SecretStore {
    /// Store `key` under `account`, replacing any existing value.
    fn store(&self, account: &str, key: &str) -> Result<()>;

    /// Read `account`, or `None` when nothing is stored for it.
    fn load(&self, account: &str) -> Result<Option<String>>;

    /// Remove `account` if it is stored.
    fn delete(&self, account: &str) -> Result<()>;
}

/// The login keychain, filed under [`SERVICE`].
pub struct Keychain;

impl SecretStore for Keychain {
    fn store(&self, account: &str, key: &str) -> Result<()> {
        store(account, key)
    }

    fn load(&self, account: &str) -> Result<Option<String>> {
        load(account)
    }

    fn delete(&self, account: &str) -> Result<()> {
        delete(account)
    }
}

impl<T: SecretStore + ?Sized> SecretStore for Arc<T> {
    fn store(&self, account: &str, key: &str) -> Result<()> {
        (**self).store(account, key)
    }

    fn load(&self, account: &str) -> Result<Option<String>> {
        (**self).load(account)
    }

    fn delete(&self, account: &str) -> Result<()> {
        (**self).delete(account)
    }
}

/// Keys held in memory for the life of the process: a test's store, and a scripted run's.
///
/// Nothing reaches the disk or the login keychain, so a test that saves a key can never leave a
/// secret behind, and an assertion asks the same trait the app does. The values are dropped when
/// the process ends, by design.
#[derive(Default)]
pub struct MemorySecrets(Mutex<HashMap<String, String>>);

impl MemorySecrets {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether `account` holds a key, for an assertion that never reads the secret back.
    pub fn holds(&self, account: &str) -> bool {
        self.0.lock().is_ok_and(|keys| keys.contains_key(account))
    }
}

impl SecretStore for MemorySecrets {
    fn store(&self, account: &str, key: &str) -> Result<()> {
        if let Ok(mut keys) = self.0.lock() {
            keys.insert(account.to_string(), key.to_string());
        }
        Ok(())
    }

    fn load(&self, account: &str) -> Result<Option<String>> {
        Ok(self
            .0
            .lock()
            .ok()
            .and_then(|keys| keys.get(account).cloned()))
    }

    fn delete(&self, account: &str) -> Result<()> {
        if let Ok(mut keys) = self.0.lock() {
            keys.remove(account);
        }
        Ok(())
    }
}

/// Store `key` under `account` in the login keychain, replacing any existing value.
pub fn store(account: &str, key: &str) -> Result<()> {
    entry(account)?.set_password(key)?;
    Ok(())
}

/// Read `account` from the login keychain, or `None` when nothing is stored for it.
pub fn load(account: &str) -> Result<Option<String>> {
    match entry(account)?.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Remove `account` from the login keychain if it is stored.
pub fn delete(account: &str) -> Result<()> {
    match entry(account)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn entry(account: &str) -> Result<keyring::Entry> {
    Ok(keyring::Entry::new(SERVICE, account)?)
}
