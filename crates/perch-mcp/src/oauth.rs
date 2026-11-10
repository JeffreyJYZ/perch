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

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use base64::Engine;
use oauth2::TokenResponse;
use rmcp::transport::auth::{AuthError, AuthorizationManager, CredentialStore, StoredCredentials};
use sha2::{Digest, Sha256};
use url::Url;

use crate::{Error, Result};

/// How long a person has to finish a browser consent screen.
///
/// This is the one bound in the crate that is not [`crate::REQUEST_TIMEOUT`]: a machine
/// answering a request gets 30 seconds, a human reading a consent screen gets five minutes,
/// and an abandoned flow still ends rather than holding a connection forever.
pub const AUTHORIZATION_TIMEOUT: Duration = Duration::from_secs(300);

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

/// What the browser came back with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Callback {
    /// The authorization code to exchange.
    pub code: String,
    /// The CSRF state, which `rmcp` keys the stored PKCE verifier by.
    pub state: String,
}

/// A loopback listener for the OAuth redirect.
pub struct Loopback {
    listener: TcpListener,
}

impl Loopback {
    /// Listen on an ephemeral port on the loopback interface.
    ///
    /// Loopback and an ephemeral port are both deliberate: the redirect never leaves this
    /// machine, and nothing has to be reserved ahead of time for the registration.
    pub fn bind() -> Result<Self> {
        Ok(Self {
            listener: TcpListener::bind(("127.0.0.1", 0))?,
        })
    }

    /// The redirect URI to register, `http://127.0.0.1:<port>/callback`.
    pub fn redirect_uri(&self) -> Result<String> {
        let port = self.listener.local_addr()?.port();
        Ok(format!("http://127.0.0.1:{port}/callback"))
    }

    /// Wait for the redirect, answer the browser, and return what it carried.
    ///
    /// Bounded by [`AUTHORIZATION_TIMEOUT`]; the accept loop is a poll rather than a blocking
    /// call so the deadline is real.
    pub async fn wait(self) -> Result<Callback> {
        tokio::task::spawn_blocking(move || self.wait_blocking(AUTHORIZATION_TIMEOUT))
            .await
            .map_err(|error| Error::Config(format!("callback listener failed: {error}")))?
    }

    fn wait_blocking(self, timeout: Duration) -> Result<Callback> {
        self.listener.set_nonblocking(true)?;
        let deadline = Instant::now() + timeout;
        let mut stream = loop {
            if Instant::now() >= deadline {
                return Err(Error::Config(format!(
                    "no authorization callback within {}s",
                    timeout.as_secs()
                )));
            }
            match self.listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(error) => return Err(error.into()),
            }
        };

        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        let request_line = {
            let mut reader = BufReader::new(&mut stream);
            let mut line = String::new();
            reader.read_line(&mut line)?;
            line
        };
        let callback = parse_callback(&request_line);

        // Answer before propagating a bad callback: the person in the browser should see that
        // the flow ended, not a connection reset.
        let body = "<!doctype html><meta charset=\"utf-8\"><title>perch</title>\
			<p>Authorization finished. You can close this window.";
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\n\
			 content-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        callback
    }
}

/// Read `code` and `state` out of the request line the browser sent.
///
/// The line looks like `GET /callback?code=abc&state=xyz HTTP/1.1`; query parsing goes through
/// `url` so percent escapes are decoded rather than handed to the token endpoint raw.
pub fn parse_callback(request_line: &str) -> Result<Callback> {
    let target = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| Error::Config(format!("malformed callback request {request_line:?}")))?;
    let url = Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(|error| Error::Config(format!("malformed callback target {target:?}: {error}")))?;

    let mut code = None;
    let mut state = None;
    let mut refusal = None;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.to_string()),
            "state" => state = Some(value.to_string()),
            "error" => refusal = Some(value.to_string()),
            _ => {}
        }
    }

    if let Some(refusal) = refusal {
        return Err(Error::Config(format!(
            "the authorization server refused: {refusal}"
        )));
    }
    let code = code.ok_or_else(|| Error::Config("the callback carried no code".to_string()))?;
    let state = state.ok_or_else(|| Error::Config("the callback carried no state".to_string()))?;
    Ok(Callback { code, state })
}

/// The PKCE `code_challenge` for a verifier: base64url of its SHA-256, unpadded.
pub fn pkce_s256(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

/// Discover the server's authorization server, register this client, and build the URL.
///
/// The returned manager holds the PKCE verifier and the pending `state`; [`finish`] needs the
/// same manager, which is why the two are separate calls.
pub async fn start(
    server_url: &str,
    server: &str,
    redirect_uri: &str,
) -> Result<(AuthorizationManager, String)> {
    start_with_store(server_url, redirect_uri, KeychainStore::new(server)).await
}

/// [`start`] with the credential store supplied, so a test can keep tokens in memory instead
/// of writing them into the machine's keychain.
pub async fn start_with_store<S: CredentialStore + 'static>(
    server_url: &str,
    redirect_uri: &str,
    store: S,
) -> Result<(AuthorizationManager, String)> {
    let mut manager = AuthorizationManager::new(server_url)
        .await
        .map_err(|error| Error::Config(format!("oauth discovery failed: {error}")))?;

    // `AuthorizationManager::new` only records the server URL; metadata discovery is a
    // separate, explicit step, and without it registration has nothing to register with.
    let resolution = manager
        .resolve_metadata()
        .await
        .map_err(|error| Error::Config(format!("oauth metadata discovery failed: {error}")))?;
    manager.set_metadata(resolution.metadata);
    manager.set_credential_store(store);

    // Dynamic client registration: perch has no pre-registered client id anywhere, so the
    // authorization server has to mint one for this install.
    let scopes: [&str; 0] = [];
    manager
        .register_client("perch", redirect_uri, &scopes)
        .await
        .map_err(|error| Error::Config(format!("dynamic client registration failed: {error}")))?;

    let url = manager
        .get_authorization_url(&scopes)
        .await
        .map_err(|error| {
            Error::Config(format!("could not build the authorization url: {error}"))
        })?;
    Ok((manager, url))
}

/// Exchange the callback for tokens, save them in the keychain, and answer the access token.
pub async fn finish(manager: &AuthorizationManager, callback: &Callback) -> Result<String> {
    manager
        .exchange_code_for_token(&callback.code, &callback.state)
        .await
        .map_err(|error| Error::Config(format!("token exchange failed: {error}")))?;
    manager
        .get_access_token()
        .await
        .map_err(|error| Error::Config(format!("no access token after exchange: {error}")))
}

/// The whole flow, including the browser: bind a callback port, register, open the
/// authorization URL, wait for the redirect, exchange, and answer the token.
pub async fn access_token(server_url: &str, server: &str) -> Result<String> {
    let loopback = Loopback::bind()?;
    let redirect_uri = loopback.redirect_uri()?;
    let (manager, url) = start(server_url, server, &redirect_uri).await?;
    open_browser(&url)?;
    let callback = loopback.wait().await?;
    finish(&manager, &callback).await
}

/// Hand the authorization URL to the system browser.
///
/// macOS only, because perch is: `open` is the one thing every browser on this machine
/// answers to without the app knowing which browser is installed.
pub fn open_browser(url: &str) -> Result<()> {
    std::process::Command::new("open")
        .arg(url)
        .spawn()
        .map(|_child| ())
        .map_err(|error| Error::Config(format!("could not open a browser: {error}")))
}
