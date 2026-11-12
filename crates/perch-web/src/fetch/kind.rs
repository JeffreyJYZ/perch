//! Telling a readable body from one that only looks readable.
//!
//! A server's own label is the first thing to trust and the last thing to believe: `text/html`
//! with a NUL in it is not text, and an unlabelled body may be either. These four questions are
//! split from the tool because each is a rule about bytes, not about fetching.

/// How many bytes at the front decide whether a body is text at all.
pub(super) const SNIFF: usize = 1024;

/// The declared type without its parameters, so `text/html; charset=utf-8` is `text/html`.
pub(super) fn media_type(content_type: &str) -> String {
    content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

/// Whether a body with this declared type is readable text. An unlabelled body is given the
/// benefit of the doubt here and judged by its bytes instead.
pub(super) fn texty(content_type: &str) -> bool {
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
pub(super) fn binary(body: &[u8]) -> bool {
    body[..body.len().min(SNIFF)].contains(&0)
}

/// Whether the body should be reduced to markdown rather than handed over as it arrived.
pub(super) fn is_html(content_type: &str, body: &[u8]) -> bool {
    if content_type.contains("html") {
        return true;
    }
    if !content_type.is_empty() {
        return false;
    }
    let head = String::from_utf8_lossy(&body[..body.len().min(512)]).to_ascii_lowercase();
    head.contains("<html") || head.contains("<!doctype html")
}
