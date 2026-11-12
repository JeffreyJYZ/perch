//! Where the app keeps provider keys: one JSON file only this user can read.
//!
//! OpenCode keeps its provider credentials the same way, in `auth.json`, and this app follows it:
//! no keychain, no per-binary approval, nothing that can answer "not stored" to the binary that
//! needs the key. The file is `secrets.json` in the app's data directory, written `0600` through a
//! temporary file so a crash mid-write cannot truncate it.

use std::path::Path;

use perch_store::secret::FileSecrets;
use perch_store::secret::SecretStore;

/// The provider keys as this app keeps them.
pub struct Secrets {
    file: FileSecrets,
}

impl Secrets {
    /// The store inside the app's data directory.
    pub fn at(dir: &Path) -> Secrets {
        Secrets {
            file: FileSecrets::new(dir.join("secrets.json")),
        }
    }
}

impl SecretStore for Secrets {
    fn store(&self, account: &str, key: &str) -> perch_store::Result<()> {
        self.file.store(account, key)
    }

    fn load(&self, account: &str) -> perch_store::Result<Option<String>> {
        self.file.load(account)
    }

    fn delete(&self, account: &str) -> perch_store::Result<()> {
        self.file.delete(account)
    }
}
