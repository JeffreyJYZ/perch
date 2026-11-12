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

/// Secrets in one JSON file only this user can read.
///
/// The login keychain asks for approval per binary, and every local build of this app is a new,
/// ad-hoc-signed binary — so a key the keychain holds was answered "not stored" by the app that
/// needed it, and every turn went out with no authorization header at all. A `0600` file has no
/// such identity: it is read by whoever runs as this user, which is exactly who may read it.
/// OpenCode keeps its provider credentials the same way, in `auth.json`.
pub struct FileSecrets {
    path: std::path::PathBuf,
}

impl FileSecrets {
    /// The store kept at `path`, which is created on the first write.
    pub fn new(path: impl Into<std::path::PathBuf>) -> FileSecrets {
        FileSecrets { path: path.into() }
    }

    /// The map the file holds, or an empty one when it holds nothing readable.
    fn read(&self) -> HashMap<String, String> {
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Writes `keys` through a temporary file, so a crash mid-write cannot truncate the store.
    fn write(&self, keys: &HashMap<String, String>) -> Result<()> {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let text = serde_json::to_string_pretty(keys)?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Created 0600 from the first byte: a file made readable and then chmodded is a file that
        // was briefly open to everyone.
        let temp = self.path.with_extension("json.tmp");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temp)?;
        file.write_all(text.as_bytes())?;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        drop(file);
        std::fs::rename(&temp, &self.path)?;
        Ok(())
    }
}

impl SecretStore for FileSecrets {
    fn store(&self, account: &str, key: &str) -> Result<()> {
        let mut keys = self.read();
        keys.insert(account.to_string(), key.to_string());
        self.write(&keys)
    }

    fn load(&self, account: &str) -> Result<Option<String>> {
        Ok(self.read().get(account).cloned())
    }

    fn delete(&self, account: &str) -> Result<()> {
        let mut keys = self.read();
        keys.remove(account);
        self.write(&keys)
    }
}
