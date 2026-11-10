//! Provider secrets live in the keychain, never in TOML, a session file or a log line.

use crate::Result;

/// The keychain service every perch secret is filed under.
pub const SERVICE: &str = "dev.jyz.perch";

/// Store `key` under `account`, replacing any existing value.
pub fn store(account: &str, key: &str) -> Result<()> {
    entry(account)?.set_password(key)?;
    Ok(())
}

/// Read `account`, or `None` when nothing is stored for it.
pub fn load(account: &str) -> Result<Option<String>> {
    match entry(account)?.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Remove `account` if it is stored.
pub fn delete(account: &str) -> Result<()> {
    match entry(account)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn entry(account: &str) -> Result<keyring::Entry> {
    Ok(keyring::Entry::new(SERVICE, account)?)
}
