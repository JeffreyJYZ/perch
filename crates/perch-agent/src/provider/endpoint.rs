//! The provider a session talks to, readable and changeable while the session runs.
//!
//! An [`Endpoint`] is a handle, not a value: every clone shares one address and one key, so the
//! settings a person saves reach the very next request of a session that is already open. Without
//! this, a key typed into Settings after the window opened reached only the sessions created
//! after it, and every turn in the open one was answered `401 Missing Authentication header`
//! with a key the keychain was holding all along.

use std::sync::{Arc, RwLock};

use crate::provider::secret::SecretString;

/// The address and key one agent talks to.
#[derive(Clone, Default)]
pub struct Endpoint {
    held: Arc<RwLock<Pair>>,
}

/// What a save replaces, together, so a request never reads half of one settings file.
#[derive(Clone, Default)]
struct Pair {
    base_url: String,
    api_key: String,
}

impl Endpoint {
    /// A handle holding `base_url` and `api_key`.
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Endpoint {
        Endpoint {
            held: Arc::new(RwLock::new(Pair {
                base_url: base_url.into(),
                api_key: api_key.into(),
            })),
        }
    }

    /// The root a request started now would post to.
    pub fn base_url(&self) -> String {
        self.read().base_url.clone()
    }

    /// The key a request started now would send, which is empty when none was saved.
    pub fn api_key(&self) -> SecretString {
        SecretString::from(self.read().api_key.clone())
    }

    /// Replaces both. A request already out keeps what it was sent with.
    pub fn set(&self, base_url: impl Into<String>, api_key: impl Into<String>) {
        let mut held = match self.held.write() {
            Ok(held) => held,
            // A poisoned lock still holds the pair; nothing here panics while holding it.
            Err(poisoned) => poisoned.into_inner(),
        };
        *held = Pair {
            base_url: base_url.into(),
            api_key: api_key.into(),
        };
    }

    /// Whether no key is held, which is the one thing that makes a provider unreachable.
    pub fn missing_key(&self) -> bool {
        self.read().api_key.trim().is_empty()
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, Pair> {
        match self.held.read() {
            Ok(held) => held,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl std::fmt::Debug for Endpoint {
    /// The address, never the key: a handle that prints its secret is a secret in a log line.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Endpoint")
            .field("base_url", &self.read().base_url)
            .field("api_key", &"…")
            .finish()
    }
}
