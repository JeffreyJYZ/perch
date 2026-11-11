//! `websearch`: titles, URLs and snippets from a public endpoint, with no API key.

mod encode;
mod parse;
#[cfg(test)]
mod tests;

use std::fmt::Write as _;
use std::sync::Arc;

use perch_tools::{Error, Result, Tool, ToolCtx, ToolOutput, ToolSpec};
use serde_json::{Value, json};

use crate::net::{FetchError, Fetcher, Http};

/// How many results a call returns unless it asks for another number.
pub const SEARCH_COUNT_DEFAULT: usize = 8;
/// The most any call may ask for.
pub const SEARCH_COUNT_MAX: usize = 20;
/// The endpoint that answers without a key and returns plain HTML.
pub const ENDPOINT: &str = "https://html.duckduckgo.com/html/?q=";
/// How much of an unexpected body is quoted back to the model.
const QUOTE: usize = 200;

pub struct WebSearch {
    fetcher: Arc<dyn Fetcher>,
    endpoint: String,
}

impl WebSearch {
    pub fn new() -> Self {
        Self { fetcher: Arc::new(Http::new()), endpoint: ENDPOINT.to_string() }
    }

    /// A tool whose transport is the caller's, which is how a test stays offline.
    pub fn with_fetcher(fetcher: Arc<dyn Fetcher>) -> Self {
        Self { fetcher, endpoint: ENDPOINT.to_string() }
    }
}

impl Default for WebSearch {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for WebSearch {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "websearch".into(),
            description: "Search the web and return titles, URLs and snippets. Answers up to 20 \
                          results (8 by default), deduplicated by URL. When the endpoint returns \
                          no results or a challenge, the error says so and quotes what came back \
                          rather than reporting an empty search."
                .into(),
            parameters: json!({
                "type": "object",
                "description": "Search the web.",
                "properties": {
                    "query": { "type": "string", "description": "What to search for." },
                    "count": {
                        "type": "integer",
                        "description": "How many results, capped at 20; defaults to 8.",
                    },
                },
                "required": ["query"],
                "additionalProperties": false,
            }),
        }
    }

    fn needs_approval(&self) -> bool {
        false
    }

    fn call(&self, args: &Value, _ctx: &ToolCtx) -> Result<ToolOutput> {
        let query = query_arg(args)?;
        let count = count_arg(args)?;
        let url = format!("{}{}", self.endpoint, encode::query(&query));
        let response = self.fetcher.get(&url).map_err(|error: FetchError| {
            Error::Message(error.to_string())
        })?;
        let page = String::from_utf8_lossy(&response.body);

        if !(200..300).contains(&response.status) {
            return Err(Error::Message(format!(
                "{url} answered HTTP {} instead of a result page: {}",
                response.status,
                quote(&page)
            )));
        }
        let hits = parse::dedupe(parse::results(&page));
        if hits.is_empty() {
            return Err(Error::Message(format!(
                "{url} answered HTTP {} with no parseable results: {}",
                response.status,
                quote(&page)
            )));
        }

        let mut text = String::new();
        for (index, hit) in hits.iter().take(count).enumerate() {
            let _ = writeln!(text, "{}. {}", index + 1, hit.title);
            let _ = writeln!(text, "   {}", hit.url);
            if !hit.snippet.is_empty() {
                let _ = writeln!(text, "   {}", hit.snippet);
            }
            text.push('\n');
        }
        Ok(ToolOutput::text(text.trim_end().to_string() + "\n"))
    }
}

fn query_arg(args: &Value) -> Result<String> {
    args.get("query")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|query| !query.is_empty())
        .map(str::to_string)
        .ok_or_else(|| Error::Message("missing string argument `query`".into()))
}

fn count_arg(args: &Value) -> Result<usize> {
    match args.get("count") {
        None | Some(Value::Null) => Ok(SEARCH_COUNT_DEFAULT),
        Some(value) => {
            let asked = value
                .as_u64()
                .ok_or_else(|| Error::Message("`count` must be a whole number".into()))?;
            Ok((asked as usize).clamp(1, SEARCH_COUNT_MAX))
        }
    }
}

/// A body quoted back on its first line, so a challenge page cannot flood the transcript.
fn quote(page: &str) -> String {
    let mut flat = String::with_capacity(QUOTE);
    for word in page.split_whitespace() {
        if !flat.is_empty() {
            flat.push(' ');
        }
        flat.push_str(word);
    }
    crate::leading(&flat, QUOTE).to_string()
}
