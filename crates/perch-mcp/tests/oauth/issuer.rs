//! A minimal OAuth 2.1 authorization server: exactly the endpoints the flow touches.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

pub struct Issuer {
    base: String,
    requests: Arc<Mutex<Vec<String>>>,
}

impl Issuer {
    pub async fn start() -> Self {
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

    pub fn mcp_url(&self) -> String {
        format!("{}/mcp", self.base)
    }

    pub fn requests(&self) -> Vec<String> {
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
