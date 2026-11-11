//! The models a provider offers: `GET <base_url>/models`.
//!
//! The list is the provider's own, sorted and deduplicated and nothing else: no preferred model,
//! no free-model rule, because a provider that returns one id returns one id. The one GET goes
//! through [`Transport`], so the picker's test answers it from a table and opens no socket.
//!
//! OpenAI-compatible gateways answer `data[].id`; a few (Ollama among them) answer
//! `models[].name`. Both are read, so a provider is not refused for its field name.

use std::collections::BTreeSet;
use std::time::Duration;

use serde::Deserialize;

use crate::{Error, Result};

/// A listing's deadline. Shorter than a turn's on purpose: a picker that waits is a picker that
/// hangs, and a model list is not worth a two-minute freeze.
pub const LIST_TIMEOUT: Duration = Duration::from_secs(10);

/// How much of a refused listing's body is kept.
const ERROR_BODY_LIMIT: usize = 2048;

/// One GET of a model list, so the real client and a test's table are interchangeable.
pub trait Transport: Send + Sync {
    /// Fetch `url`, sending `api_key` as a bearer token when it is not empty.
    fn get(&self, url: &str, api_key: &str) -> Result<Vec<u8>>;
}

/// The real transport: one blocking GET on the caller's thread.
pub struct Http;

impl Transport for Http {
    fn get(&self, url: &str, api_key: &str) -> Result<Vec<u8>> {
        let client = reqwest::blocking::Client::builder()
            .timeout(LIST_TIMEOUT)
            .build()?;
        let mut request = client.get(url).header("accept", "application/json");
        if !api_key.is_empty() {
            request = request.bearer_auth(api_key);
        }
        let response = request.send()?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(Error::Status {
                status: status.as_u16(),
                body: truncate(body),
            });
        }
        Ok(response.bytes()?.to_vec())
    }
}

/// The ids `base_url` lists, through the real client.
pub fn list_models(base_url: &str, api_key: &str) -> Result<Vec<String>> {
    list_models_with(&Http, base_url, api_key)
}

/// The ids `base_url` lists, through `transport`.
pub fn list_models_with(
    transport: &dyn Transport,
    base_url: &str,
    api_key: &str,
) -> Result<Vec<String>> {
    let url = listing_url(base_url)?;
    let body = transport.get(&url, api_key)?;
    parse(&body)
}

/// The listing URL for `base`, which may or may not already end in the chat path.
fn listing_url(base: &str) -> Result<String> {
    let base = base.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err(Error::Config("base_url is empty".to_string()));
    }
    if !(base.starts_with("http://") || base.starts_with("https://")) {
        return Err(Error::Config(format!(
            "base_url is not an http url: {base}"
        )));
    }
    let root = base
        .strip_suffix("/chat/completions")
        .unwrap_or(base)
        .trim_end_matches('/');
    Ok(format!("{root}/models"))
}

/// The listing body in either shape.
#[derive(Debug, Deserialize)]
struct WireList {
    #[serde(default)]
    data: Vec<WireModel>,
    #[serde(default)]
    models: Vec<WireModel>,
}

#[derive(Debug, Deserialize)]
struct WireModel {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

/// The ids in `body`, trimmed, deduplicated and sorted.
fn parse(body: &[u8]) -> Result<Vec<String>> {
    let list: WireList = serde_json::from_slice(body)?;
    let ids: BTreeSet<String> = list
        .data
        .into_iter()
        .filter_map(|model| model.id)
        .chain(list.models.into_iter().filter_map(|model| model.name))
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();
    Ok(ids.into_iter().collect())
}

/// Keep the head of a provider error body; an unbounded body is a memory sink on a hostile server.
fn truncate(text: String) -> String {
    if text.len() <= ERROR_BODY_LIMIT {
        return text;
    }
    let mut cut = ERROR_BODY_LIMIT;
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}\n[truncated {} bytes]", &text[..cut], text.len() - cut)
}
