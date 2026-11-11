//! `webfetch` end to end, answered from fixtures so no test touches the network.

use std::path::PathBuf;
use std::sync::Arc;

use perch_tools::{Error, Tool, ToolCtx, ToolOutput};
use serde_json::json;

use super::{FETCH_MAX_BYTES, WebFetch};
use crate::net::{FetchError, MemoryFetcher, Response};

fn ctx() -> ToolCtx {
    ToolCtx { root: PathBuf::from("/") }
}

fn answer(status: u16, content_type: &str, body: &str) -> Response {
    Response {
        status,
        url: "https://example.test/page".into(),
        content_type: content_type.into(),
        body: body.as_bytes().to_vec(),
    }
}

fn tool(fetcher: MemoryFetcher) -> (WebFetch, Arc<MemoryFetcher>) {
    let fetcher = Arc::new(fetcher);
    (WebFetch::with_fetcher(fetcher.clone()), fetcher)
}

fn text(output: ToolOutput) -> String {
    output.text
}

fn message(error: Error) -> String {
    match error {
        Error::Message(words) => words,
        other => panic!("expected a plain message, got {other:?}"),
    }
}

#[test]
fn a_page_becomes_readable_markdown_with_its_final_url_and_size() {
    let body = "<h2>Title</h2><p>Hello <a href=\"https://x.test/a\">link</a>.</p>";
    let mut response = answer(200, "text/html; charset=utf-8", body);
    response.url = "https://example.test/final".into();
    let (tool, _) = tool(MemoryFetcher::new().answer("example.test/page", response));

    let out = text(tool.call(&json!({"url": "https://example.test/page"}), &ctx()).unwrap());
    assert!(out.starts_with("https://example.test/final (HTTP 200, "), "{out}");
    assert!(out.contains(&format!("{} bytes", body.len())), "{out}");
    assert!(out.contains("## Title"), "{out}");
    assert!(out.contains("Hello link (https://x.test/a)."), "{out}");
}

#[test]
fn text_format_drops_markdown_and_html_format_returns_the_body_verbatim() {
    let body = "<h1>Head</h1><p><a href=\"https://x.test\">label</a></p>";
    let plain = MemoryFetcher::new().answer("x.test", answer(200, "text/html", body));
    let (plain_tool, _) = tool(plain);
    let out = text(
        plain_tool
            .call(&json!({"url": "https://x.test", "format": "text"}), &ctx())
            .unwrap(),
    );
    assert!(out.contains("Head") && !out.contains('#') && !out.contains("https://x.test"), "{out}");

    let raw = MemoryFetcher::new().answer("x.test", answer(200, "text/html", body));
    let (raw_tool, _) = tool(raw);
    let out = text(
        raw_tool
            .call(&json!({"url": "https://x.test", "format": "html"}), &ctx())
            .unwrap(),
    );
    assert!(out.ends_with(body), "{out}");
}

#[test]
fn a_binary_content_type_names_itself_and_never_the_body() {
    let (tool, _) = tool(MemoryFetcher::new().answer(
        "x.test",
        answer(200, "image/png", "\u{0}PNG-secret-bytes"),
    ));
    let error = message(tool.call(&json!({"url": "https://x.test/a.png"}), &ctx()).unwrap_err());
    assert_eq!(error, "refusing a non-text body: image/png");
    assert!(!error.contains("secret"), "{error}");
}

#[test]
fn a_nul_in_the_first_kib_refuses_even_a_text_content_type() {
    let mut body = String::new();
    for _ in 0..32 {
        body.push('\u{0}');
    }
    let (tool, _) = tool(MemoryFetcher::new().answer("x.test", answer(200, "text/plain", &body)));
    let error = message(tool.call(&json!({"url": "https://x.test"}), &ctx()).unwrap_err());
    assert_eq!(error, "refusing a non-text body: text/plain");
}

#[test]
fn a_non_2xx_status_is_a_message_naming_the_status_not_a_panic() {
    let (tool, _) = tool(MemoryFetcher::new().answer("x.test", answer(404, "text/html", "gone")));
    let error = message(tool.call(&json!({"url": "https://x.test/missing"}), &ctx()).unwrap_err());
    assert_eq!(error, "https://x.test/missing answered HTTP 404");
}

#[test]
fn text_past_the_cap_is_cut_at_a_character_boundary_and_the_cut_is_named() {
    let body = "<p>".to_string() + &"é".repeat(FETCH_MAX_BYTES) + "</p>";
    let (tool, _) = tool(MemoryFetcher::new().answer("x.test", answer(200, "text/html", &body)));
    let out = text(tool.call(&json!({"url": "https://x.test"}), &ctx()).unwrap());
    let (head, note) = out.split_once("\n[truncated at ").expect("a named cut");
    assert!(note.starts_with(&format!("{FETCH_MAX_BYTES} bytes")), "{note}");
    let kept = head.split_once("\n\n").expect("header").1;
    assert!(kept.len() <= FETCH_MAX_BYTES, "kept {} bytes", kept.len());
    assert!(kept.ends_with('é'), "the cut landed mid-character");
}

#[test]
fn a_scheme_other_than_http_is_refused_before_any_request() {
    let (tool, fetcher) = tool(MemoryFetcher::new());
    let error = message(tool.call(&json!({"url": "file:///etc/passwd"}), &ctx()).unwrap_err());
    assert!(error.contains("file:///etc/passwd is not an http or https URL"), "{error}");
    assert!(fetcher.asked().is_empty(), "the transport was called anyway");
}

#[test]
fn a_transport_failure_is_handed_back_with_the_url() {
    let (tool, _) = tool(MemoryFetcher::new().fail(
        "x.test",
        FetchError::Transport { url: "https://x.test".into(), reason: "dns".into() },
    ));
    let error = message(tool.call(&json!({"url": "https://x.test"}), &ctx()).unwrap_err());
    assert_eq!(error, "the request to https://x.test failed: dns");
}

#[test]
fn an_unknown_format_or_a_missing_url_is_refused_by_name() {
    let (tool, fetcher) = tool(MemoryFetcher::new());
    let error = message(
        tool.call(&json!({"url": "https://x.test", "format": "pdf"}), &ctx())
            .unwrap_err(),
    );
    assert!(error.contains("format must be markdown, text or html, not \"pdf\""), "{error}");
    let error = message(tool.call(&json!({}), &ctx()).unwrap_err());
    assert_eq!(error, "missing string argument `url`");
    assert!(fetcher.asked().is_empty());
}

#[test]
fn a_plain_text_body_is_passed_through_untouched() {
    let body = "line one\nline two\n";
    let (tool, _) = tool(MemoryFetcher::new().answer("x.test", answer(200, "text/plain", body)));
    let out = text(tool.call(&json!({"url": "https://x.test"}), &ctx()).unwrap());
    assert!(out.ends_with(body), "{out}");
}
