//! `webfetch`: one URL, reduced to text the model can read.

#[cfg(test)]
mod tests;

use std::sync::Arc;

use perch_tools::{Error, Result, Tool, ToolCtx, ToolOutput, ToolSpec};
use serde_json::{Value, json};

use crate::html;
use crate::net::{self, FetchError, Fetcher, Http};

/// `webfetch` never hands the model more than this many bytes of text.
pub const FETCH_MAX_BYTES: usize = 64 * 1024;
/// A body is sniffed for binary content in this many leading bytes.
const SNIFF: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Markdown,
    Text,
    Html,
}

pub struct WebFetch {
    fetcher: Arc<dyn Fetcher>,
}

impl WebFetch {
    pub fn new() -> Self {
        Self { fetcher: Arc::new(Http::new()) }
    }

    /// A tool whose transport is the caller's, which is how a test stays offline.
    pub fn with_fetcher(fetcher: Arc<dyn Fetcher>) -> Self {
        Self { fetcher }
    }
}

impl Default for WebFetch {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for WebFetch {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "webfetch".into(),
            description: "Fetch one http or https URL and return its readable text. HTML becomes \
                          markdown by default: headings as `#`, links as `text (url)`. `text` \
                          drops the markdown syntax, `html` returns the body as served. The answer \
                          names the final URL after redirects, the status and the body size, and \
                          the text is capped at 64 KiB."
                .into(),
            parameters: json!({
                "type": "object",
                "description": "Fetch one URL.",
                "properties": {
                    "url": { "type": "string", "description": "An absolute http or https URL." },
                    "format": {
                        "type": "string",
                        "enum": ["markdown", "text", "html"],
                        "description": "How to reduce the body; defaults to markdown.",
                    },
                },
                "required": ["url"],
                "additionalProperties": false,
            }),
        }
    }

    fn needs_approval(&self) -> bool {
        false
    }

    fn call(&self, args: &Value, _ctx: &ToolCtx) -> Result<ToolOutput> {
        let url = url_arg(args)?;
        let format = format_arg(args)?;
        net::check_url(&url).map_err(refused)?;
        let response = self.fetcher.get(&url).map_err(refused)?;

        if !(200..300).contains(&response.status) {
            return Err(Error::Message(format!("{url} answered HTTP {}", response.status)));
        }
        let content_type = media_type(&response.content_type);
        if !texty(&content_type) || binary(&response.body) {
            return Err(Error::Message(format!("refusing a non-text body: {content_type}")));
        }

        let raw = String::from_utf8_lossy(&response.body);
        let text = match format {
            Format::Html => raw.to_string(),
            Format::Markdown if is_html(&content_type, &response.body) => {
                html::reduce(&raw, html::MARKDOWN)
            }
            Format::Text if is_html(&content_type, &response.body) => {
                html::reduce(&raw, html::PLAIN)
            }
            Format::Markdown | Format::Text => raw.to_string(),
        };

        let bytes = response.body.len();
        let mut text = text;
        let mut note = String::new();
        if text.len() > FETCH_MAX_BYTES {
            let cut = crate::leading(&text, FETCH_MAX_BYTES).len();
            text.truncate(cut);
            note = format!(
                "\n[truncated at {FETCH_MAX_BYTES} bytes of text; the body was {bytes} bytes]"
            );
        }
        Ok(ToolOutput::text(format!(
            "{} (HTTP {}, {} bytes)\n\n{text}{note}",
            response.url, response.status, bytes
        )))
    }
}

fn url_arg(args: &Value) -> Result<String> {
    let url = args
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .ok_or_else(|| Error::Message("missing string argument `url`".into()))?;
    Ok(url.to_string())
}

fn format_arg(args: &Value) -> Result<Format> {
    match args.get("format").and_then(Value::as_str) {
        None | Some("markdown") => Ok(Format::Markdown),
        Some("text") => Ok(Format::Text),
        Some("html") => Ok(Format::Html),
        Some(other) => Err(Error::Message(format!(
            "format must be markdown, text or html, not {other:?}"
        ))),
    }
}

fn refused(error: FetchError) -> Error {
    Error::Message(error.to_string())
}

/// The declared type without its parameters, so `text/html; charset=utf-8` is `text/html`.
fn media_type(content_type: &str) -> String {
    content_type.split(';').next().unwrap_or("").trim().to_ascii_lowercase()
}

/// Whether a body with this declared type is readable text. An unlabelled body is given the
/// benefit of the doubt here and judged by its bytes instead.
fn texty(content_type: &str) -> bool {
    if content_type.is_empty() {
        return true;
    }
    content_type.starts_with("text/")
        || content_type.ends_with("+json")
        || content_type.ends_with("+xml")
        || matches!(
            content_type,
            "application/json"
                | "application/xml"
                | "application/javascript"
                | "application/x-javascript"
                | "application/xhtml+xml"
                | "application/rss+xml"
                | "application/atom+xml"
                | "application/ld+json"
                | "application/x-ndjson"
                | "application/graphql"
        )
}

/// A NUL in the first [`SNIFF`] bytes is the signature of a body that is not text, whatever the
/// server labelled it.
fn binary(body: &[u8]) -> bool {
    body[..body.len().min(SNIFF)].contains(&0)
}

fn is_html(content_type: &str, body: &[u8]) -> bool {
    if content_type.contains("html") {
        return true;
    }
    if !content_type.is_empty() {
        return false;
    }
    let head = String::from_utf8_lossy(&body[..body.len().min(512)]).to_ascii_lowercase();
    head.contains("<html") || head.contains("<!doctype html")
}
