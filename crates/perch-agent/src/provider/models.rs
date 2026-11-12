//! The models a provider offers: `GET <base_url>/models`.
//!
//! The list is the provider's own, sorted and deduplicated and nothing else: no preferred model,
//! no free-model rule. The one GET goes through [`Transport`], so the picker's test answers it
//! from a table and opens no socket. Gateways answer `data[].id`, a few (Ollama among them)
//! `models[].name`; both are read, so a provider is not refused for its field name.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::Deserialize;

use super::price::WirePricing;
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

/// One model a provider offers: its id, the context window it states, and what it charges.
///
/// The window and the prices travel with the id because the meter needs both: a list of bare ids
/// would make it fetch the same payload again just to learn two numbers. `context_length` and
/// `pricing` are the fields OpenRouter and its imitators state, in tokens and dollars per token;
/// a provider that states neither leaves both unknown, so the meter shows a token count with no
/// share and no money.
#[derive(Debug, Clone, PartialEq)]
pub struct ListedModel {
    pub id: String,
    /// The context window in tokens, when the provider states one.
    pub context: Option<u32>,
    /// Dollars per input token, when the provider states it.
    pub prompt_price: Option<f64>,
    /// Dollars per output token, when the provider states it.
    pub completion_price: Option<f64>,
}

/// The ids `base_url` lists, through the real client.
pub fn list_models(base_url: &str, api_key: &str) -> Result<Vec<ListedModel>> {
    list_models_with(&Http, base_url, api_key)
}

/// The models `base_url` lists, through `transport`.
pub fn list_models_with(
    transport: &dyn Transport,
    base_url: &str,
    api_key: &str,
) -> Result<Vec<ListedModel>> {
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
    #[serde(default)]
    context_length: Option<u32>,
    #[serde(default)]
    context_window: Option<u32>,
    /// Some gateways put the limit on the serving provider instead.
    #[serde(default)]
    top_provider: Option<WireProvider>,
    /// The dollars-per-token rates, on the gateways that state them.
    #[serde(default)]
    pricing: Option<WirePricing>,
}

#[derive(Debug, Deserialize)]
struct WireProvider {
    #[serde(default)]
    context_length: Option<u32>,
}

impl WireModel {
    /// The context window this entry states, however it spells it.
    fn context(&self) -> Option<u32> {
        self.context_length
            .or(self.context_window)
            .or_else(|| self.top_provider.as_ref()?.context_length)
            .filter(|limit| *limit > 0)
    }
}

/// What one id is known to state, gathered across the copies a listing carries.
#[derive(Default)]
struct Facts {
    context: Option<u32>,
    prompt: Option<f64>,
    completion: Option<f64>,
}

/// The models in `body`, trimmed, deduplicated and sorted by id.
///
/// A duplicate entry keeps the limit and the prices whenever either copy states one: a gateway that
/// lists a model twice must not lose the numbers that make the meter work.
fn parse(body: &[u8]) -> Result<Vec<ListedModel>> {
    let list: WireList = serde_json::from_slice(body)?;
    let mut listed: BTreeMap<String, Facts> = BTreeMap::new();
    for model in list.data.into_iter().chain(list.models) {
        let prices = model.pricing.as_ref().and_then(WirePricing::values);
        let context = model.context();
        let Some(id) = model.id.or(model.name) else {
            continue;
        };
        let id = id.trim().to_string();
        if id.is_empty() {
            continue;
        }
        let facts = listed.entry(id).or_default();
        facts.context = facts.context.or(context);
        facts.prompt = facts.prompt.or(prices.map(|pair| pair.0));
        facts.completion = facts.completion.or(prices.map(|pair| pair.1));
    }
    Ok(listed
        .into_iter()
        .map(|(id, facts)| ListedModel {
            id,
            context: facts.context,
            prompt_price: facts.prompt,
            completion_price: facts.completion,
        })
        .collect())
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
