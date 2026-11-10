//! The API key, kept out of every log line and every debug dump.

use std::fmt;

/// A secret that never reaches `Debug`, a log line or a session file.
///
/// A local newtype rather than `secrecy`: the crate is not in the workspace lock, and this is
/// the only behaviour perch needs from it.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretString(String);

impl SecretString {
    /// Wrap `secret`.
    pub fn new(secret: impl Into<String>) -> Self {
        Self(secret.into())
    }

    /// The secret itself. Call it only where the value is used, never where it is printed.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    /// Whether the secret is empty, in which case no authorization header is sent.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<&str> for SecretString {
    fn from(secret: &str) -> Self {
        Self(secret.to_string())
    }
}

impl From<String> for SecretString {
    fn from(secret: String) -> Self {
        Self(secret)
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretString(<redacted>)")
    }
}
