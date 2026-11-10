//! Tokens in the keychain, one entry per server.

use oauth2::TokenResponse;
use rmcp::transport::auth::{AuthError, CredentialStore, StoredCredentials};

use crate::{Error, Result};

/// The keychain account a server's tokens are filed under.
pub fn account_for(server: &str) -> String {
    format!("mcp:{server}")
}

/// Tokens in the keychain, one entry per server, JSON under the `mcp:<server>` account.
///
/// `rmcp` decides when to load, save, refresh and clear; this only moves the bytes to and from
/// `perch_store::secret`. A keychain call is a fast local operation and this runs once per
/// connection, so it is not worth a thread hop per access.
pub struct KeychainStore {
    account: String,
}

impl KeychainStore {
    /// A store for one server's credentials.
    pub fn new(server: &str) -> Self {
        Self {
            account: account_for(server),
        }
    }
}

#[async_trait::async_trait]
impl CredentialStore for KeychainStore {
    async fn load(&self) -> std::result::Result<Option<StoredCredentials>, AuthError> {
        let stored = perch_store::secret::load(&self.account)
            .map_err(|error| AuthError::CredentialStoreError(error.to_string()))?;
        match stored {
            Some(text) => serde_json::from_str(&text).map(Some).map_err(|error| {
                AuthError::CredentialStoreError(format!(
                    "stored credentials are unreadable: {error}"
                ))
            }),
            None => Ok(None),
        }
    }

    async fn save(&self, credentials: StoredCredentials) -> std::result::Result<(), AuthError> {
        let text = serde_json::to_string(&credentials)
            .map_err(|error| AuthError::CredentialStoreError(error.to_string()))?;
        perch_store::secret::store(&self.account, &text)
            .map_err(|error| AuthError::CredentialStoreError(error.to_string()))
    }

    async fn clear(&self) -> std::result::Result<(), AuthError> {
        perch_store::secret::delete(&self.account)
            .map_err(|error| AuthError::CredentialStoreError(error.to_string()))
    }
}

/// The access token already stored for a server, if it has one.
///
/// A stored token is used without asking the user again; only its absence starts the browser
/// flow, so a restart does not re-prompt for a server that was authorized yesterday.
pub fn stored_token(server: &str) -> Result<Option<String>> {
    let Some(text) = perch_store::secret::load(&account_for(server))? else {
        return Ok(None);
    };
    let credentials: StoredCredentials = serde_json::from_str(&text).map_err(|error| {
        Error::Config(format!("credentials for {server} are unreadable: {error}"))
    })?;
    Ok(credentials
        .token_response
        .map(|token| token.access_token().secret().to_string()))
}
