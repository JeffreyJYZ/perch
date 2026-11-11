//! The one HTTP request a tool makes, behind a trait a test can answer instead.
//!
//! [`Fetcher`] is deliberately narrow: one `GET`, one body, no streaming, no cookies. Anything a
//! tool needs to know about a response (status, final URL, content type, bytes) is carried back
//! whole, so the decision about whether the body is usable lives in the tool, not in the transport.

mod http;
#[cfg(test)]
mod memory;

use std::time::Duration;

pub use http::Http;
#[cfg(test)]
pub(crate) use memory::MemoryFetcher;

/// What came back, before a tool has decided whether it can use it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    /// The URL actually answered, after any redirect the policy allowed.
    pub url: String,
    /// The `Content-Type` header verbatim, empty when the server sent none.
    pub content_type: String,
    pub body: Vec<u8>,
}

/// Why a request did not produce a [`Response`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FetchError {
    #[error("{0} is not an http or https URL")]
    Scheme(String),
    #[error("the request to {url} failed: {reason}")]
    Transport { url: String, reason: String },
    #[error("the request to {url} exceeded {after:?}")]
    Timeout { url: String, after: Duration },
}

/// One `GET` for a URL that must already be http or https.
pub trait Fetcher: Send + Sync {
    fn get(&self, url: &str) -> Result<Response, FetchError>;
}

/// Whether a string names a URL this crate will send: an http(s) scheme and a non-empty host.
///
/// Written by hand rather than parsed, so a scheme the transport would refuse is refused here
/// first, with the offending value named instead of a transport error.
pub fn check_url(url: &str) -> Result<(), FetchError> {
    let rest = match url.split_once("://") {
        Some((scheme, rest)) => {
            if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
                return Err(FetchError::Scheme(url.to_string()));
            }
            rest
        }
        None => return Err(FetchError::Scheme(url.to_string())),
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host.is_empty() || host.starts_with(':') {
        return Err(FetchError::Scheme(url.to_string()));
    }
    Ok(())
}
