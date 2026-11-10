//! OAuth 2.1 for remote MCP servers.
//!
//! The flow is the one the MCP authorization spec describes, and every step is a separate
//! function so it can be driven by a test without a browser:
//!
//! 1. [`start`] discovers the protected-resource and authorization-server metadata, registers
//!    this client dynamically, and answers the authorization URL — which carries the PKCE
//!    `S256` challenge and a CSRF `state`.
//! 2. [`Loopback`] listens on `127.0.0.1` and reads the redirect the browser makes.
//! 3. [`finish`] exchanges the code, `state` and PKCE verifier for tokens, which the
//!    authorization manager saves through [`KeychainStore`] under `mcp:<server>`.
//!
//! Tokens therefore live in the macOS keychain and never in `settings.toml`, a session file or
//! a log line. [`open_browser`] is the only step no test performs.
//!
//! The flow is split into its parts: `store` is the keychain, `loopback` is the redirect
//! listener and its request line, and `flow` is the requests to the authorization server.

mod flow;
mod loopback;
mod store;

use std::time::Duration;

pub use flow::{access_token, finish, open_browser, pkce_s256, start, start_with_store};
pub use loopback::{Callback, Loopback, parse_callback};
pub use store::{KeychainStore, account_for, stored_token};

/// How long a person has to finish a browser consent screen.
///
/// This is the one bound in the crate that is not [`crate::REQUEST_TIMEOUT`]: a machine
/// answering a request gets 30 seconds, a human reading a consent screen gets five minutes,
/// and an abandoned flow still ends rather than holding a connection forever.
pub const AUTHORIZATION_TIMEOUT: Duration = Duration::from_secs(300);
