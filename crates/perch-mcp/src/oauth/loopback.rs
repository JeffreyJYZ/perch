//! The loopback redirect listener and the request line it reads.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use url::Url;

use super::AUTHORIZATION_TIMEOUT;
use crate::{Error, Result};

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
