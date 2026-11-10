//! The requests to the authorization server, and the one browser launch.

use base64::Engine;
use rmcp::transport::auth::{AuthorizationManager, CredentialStore};
use sha2::{Digest, Sha256};

use super::loopback::{Callback, Loopback};
use super::store::KeychainStore;
use crate::{Error, Result};

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
