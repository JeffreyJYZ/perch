//! The real transport: one pooled blocking client, a deadline, and a redirect policy it owns.

use std::io::Read as IoRead;

use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use reqwest::redirect;

use super::{FetchError, Fetcher, Response};
use crate::TIMEOUT;

/// Redirects followed before the chain is refused. The response is the last 3xx then, so a tool
/// reports the status it actually got rather than a fabricated failure.
const MAX_REDIRECTS: usize = 10;
/// A body larger than this is cut off at the transport. A page the model cannot read in full is
/// not worth holding in memory, and `webfetch` caps what it returns far below this anyway.
const MAX_BODY: u64 = 8 * 1024 * 1024;
const USER_AGENT: &str = concat!("perch/", env!("CARGO_PKG_VERSION"), " (+webfetch)");

/// The network itself.
pub struct Http {
    client: Client,
}

impl Http {
    pub fn new() -> Self {
        Self { client: build() }
    }
}

impl Default for Http {
    fn default() -> Self {
        Self::new()
    }
}

impl Fetcher for Http {
    fn get(&self, url: &str) -> Result<Response, FetchError> {
        super::check_url(url)?;
        let response = self
            .client
            .get(url)
            .send()
            .map_err(|error| transport(url, &error))?;
        let status = response.status().as_u16();
        let url = response.url().to_string();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        let mut body = Vec::new();
        response
            .take(MAX_BODY)
            .read_to_end(&mut body)
            .map_err(|error| FetchError::Transport {
                url: url.clone(),
                reason: error.to_string(),
            })?;
        Ok(Response {
            status,
            url,
            content_type,
            body,
        })
    }
}

/// A client is built once and its TLS setup reused; if it cannot be built the default client is
/// the same backend, so the failure is not one a caller can act on.
fn build() -> Client {
    Client::builder()
        .timeout(TIMEOUT)
        .user_agent(USER_AGENT)
        .redirect(redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= MAX_REDIRECTS {
                attempt.stop()
            } else if super::check_url(attempt.url().as_str()).is_err() {
                // A redirect to `file:` or `ftp:` is a refusal, never a fetch.
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()
        .unwrap_or_default()
}

fn transport(url: &str, error: &reqwest::Error) -> FetchError {
    if error.is_timeout() {
        return FetchError::Timeout {
            url: url.to_string(),
            after: TIMEOUT,
        };
    }
    FetchError::Transport {
        url: url.to_string(),
        reason: error.to_string(),
    }
}
