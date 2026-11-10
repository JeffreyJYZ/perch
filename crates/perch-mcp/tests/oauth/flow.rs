//! The whole flow over a real socket, with the test playing the browser.

use std::collections::HashMap;

use perch_mcp::oauth::{self, Loopback};
use rmcp::transport::InMemoryCredentialStore;
use url::Url;

use crate::issuer::Issuer;

#[tokio::test]
async fn the_whole_flow_registers_challenges_and_exchanges() {
    let issuer = Issuer::start().await;
    let loopback = Loopback::bind().unwrap();
    let redirect_uri = loopback.redirect_uri().unwrap();

    let (manager, authorization_url) = oauth::start_with_store(
        &issuer.mcp_url(),
        &redirect_uri,
        InMemoryCredentialStore::new(),
    )
    .await
    .expect("discovery and registration");

    let parsed = Url::parse(&authorization_url).unwrap();
    let query: HashMap<String, String> = parsed.query_pairs().into_owned().collect();
    assert_eq!(query["code_challenge_method"], "S256");
    assert_eq!(query["redirect_uri"], redirect_uri);
    assert_eq!(query["client_id"], "perch-test-client");
    assert!(!query["code_challenge"].is_empty(), "no PKCE challenge");
    let state = query["state"].clone();

    // What the person's browser does: fetch the authorization endpoint, follow its redirect to
    // the loopback listener, and land back in this process.
    let waiter = tokio::spawn(async move { loopback.wait().await });
    let browser = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let authorized = browser
        .get(&authorization_url)
        .send()
        .await
        .expect("authorize");
    assert_eq!(authorized.status().as_u16(), 302);
    let location = authorized.headers()["location"]
        .to_str()
        .unwrap()
        .to_string();
    reqwest::get(&location).await.expect("callback");

    let callback = waiter
        .await
        .unwrap()
        .expect("the callback was not received");
    assert_eq!(callback.code, "test-code");
    assert_eq!(callback.state, state);

    let token = oauth::finish(&manager, &callback).await.expect("exchange");
    assert_eq!(token, "fake-token");

    // The token request proves the code, the verifier and the registered client went together.
    let requests = issuer.requests();
    assert!(
        requests
            .iter()
            .any(|line| line.starts_with("POST /register")),
        "no dynamic client registration: {requests:?}"
    );
    let exchange = requests
        .iter()
        .find(|line| line.starts_with("POST /token"))
        .unwrap_or_else(|| panic!("no token request: {requests:?}"));
    assert!(exchange.contains("code=test-code"), "{exchange}");
    assert!(
        exchange.contains("client_id=perch-test-client"),
        "{exchange}"
    );
    let verifier = exchange
        .split("code_verifier=")
        .nth(1)
        .and_then(|rest| rest.split('&').next())
        .unwrap_or_else(|| panic!("no PKCE verifier sent: {exchange}"));
    assert_eq!(
        oauth::pkce_s256(verifier),
        query["code_challenge"],
        "the verifier does not match the challenge"
    );
}
