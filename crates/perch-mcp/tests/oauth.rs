//! OAuth against a fake issuer: discovery, dynamic client registration, PKCE, the loopback
//! callback and the token exchange, all real HTTP over a real socket on this machine.
//!
//! No live authorization server and no browser are involved: the test plays the browser, which
//! is the only part of the flow a machine cannot do for itself.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use perch_mcp::oauth::{self, Callback, Loopback};
use rmcp::transport::InMemoryCredentialStore;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

/// A minimal OAuth 2.1 authorization server: exactly the endpoints the flow touches.
struct Issuer {
    base: String,
    requests: Arc<Mutex<Vec<String>>>,
}

impl Issuer {
    async fn start() -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let serving = Arc::clone(&requests);
        let served_base = base.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let base = served_base.clone();
                let log = Arc::clone(&serving);
                tokio::spawn(async move {
                    let mut buffer = vec![0u8; 16384];
                    let read = stream.read(&mut buffer).await.unwrap_or(0);
                    let request = String::from_utf8_lossy(&buffer[..read]).to_string();
                    let (head, body) = request.split_once("\r\n\r\n").unwrap_or((&request, ""));
                    let mut parts = head.lines().next().unwrap_or_default().split_whitespace();
                    let method = parts.next().unwrap_or_default().to_string();
                    let target = parts.next().unwrap_or_default().to_string();
                    log.lock()
                        .unwrap()
                        .push(format!("{method} {target} {body}"));
                    let response = respond(&base, &method, &target);
                    let _ = stream.write_all(response.as_bytes()).await;
                    let _ = stream.shutdown().await;
                });
            }
        });
        Self { base, requests }
    }

    fn mcp_url(&self) -> String {
        format!("{}/mcp", self.base)
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

fn respond(base: &str, method: &str, target: &str) -> String {
    let path = target.split('?').next().unwrap_or(target);
    match (method, path) {
        ("GET", "/.well-known/oauth-protected-resource") => json(
            200,
            &format!(r#"{{"resource":"{base}/mcp","authorization_servers":["{base}"]}}"#),
        ),
        ("GET", "/.well-known/oauth-authorization-server") => json(
            200,
            &format!(
                r#"{{"issuer":"{base}",
					"authorization_endpoint":"{base}/authorize",
					"token_endpoint":"{base}/token",
					"registration_endpoint":"{base}/register",
					"response_types_supported":["code"],
					"grant_types_supported":["authorization_code","refresh_token"],
					"code_challenge_methods_supported":["S256"],
					"token_endpoint_auth_methods_supported":["none"]}}"#
            ),
        ),
        ("POST", "/register") => json(
            201,
            r#"{"client_id":"perch-test-client","redirect_uris":[]}"#,
        ),
        ("GET", "/authorize") => {
            let url = Url::parse(&format!("{base}{target}")).unwrap();
            let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
            let redirect = query
                .get("redirect_uri")
                .expect("authorize without redirect_uri");
            let state = query.get("state").cloned().unwrap_or_default();
            let separator = if redirect.contains('?') { '&' } else { '?' };
            format!(
                "HTTP/1.1 302 Found\r\nlocation: {redirect}{separator}code=test-code&state={state}\r\n\
				 content-length: 0\r\nconnection: close\r\n\r\n"
            )
        }
        ("POST", "/token") => json(
            200,
            r#"{"access_token":"fake-token","token_type":"Bearer","expires_in":3600}"#,
        ),
        _ => json(404, r#"{"error":"not_found"}"#),
    }
}

fn json(status: u16, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\
		 connection: close\r\n\r\n{body}",
        body.len()
    )
}

/// The RFC 7636 known answer, plus the account the keychain entry is filed under.
#[test]
fn the_pure_parts_match_the_spec() {
    assert_eq!(
        oauth::pkce_s256("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
    assert_eq!(oauth::account_for("fixture"), "mcp:fixture");
    assert_eq!(
        oauth::parse_callback("GET /callback?code=abc123&state=xyz HTTP/1.1\r\n").unwrap(),
        Callback {
            code: "abc123".to_string(),
            state: "xyz".to_string(),
        }
    );
    // A refusal is an error, not an empty code handed to the token endpoint.
    assert!(
        oauth::parse_callback("GET /callback?error=access_denied&state=xyz HTTP/1.1\r\n").is_err()
    );
    assert!(oauth::parse_callback("GET /callback?code=abc123 HTTP/1.1\r\n").is_err());
}

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
