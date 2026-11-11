//! Every way `webfetch` refuses, and the one case where it hands a body over untouched.
//!
//! A refusal that quietly did something else is what these pin: a scheme that is not http is
//! never requested, a bad format never reaches the wire, and a plain body is not "cleaned".

use perch_tools::Tool;
use serde_json::json;

use super::{answer, ctx, message, text, tool};
use crate::net::{FetchError, MemoryFetcher};

#[test]
fn a_scheme_other_than_http_is_refused_before_any_request() {
    let (tool, fetcher) = tool(MemoryFetcher::new());
    let error = message(
        tool.call(&json!({"url": "file:///etc/passwd"}), &ctx())
            .unwrap_err(),
    );
    assert!(
        error.contains("file:///etc/passwd is not an http or https URL"),
        "{error}"
    );
    assert!(
        fetcher.asked().is_empty(),
        "the transport was called anyway"
    );
}

#[test]
fn a_transport_failure_is_handed_back_with_the_url() {
    let (tool, _) = tool(MemoryFetcher::new().fail(
        "x.test",
        FetchError::Transport {
            url: "https://x.test".into(),
            reason: "dns".into(),
        },
    ));
    let error = message(
        tool.call(&json!({"url": "https://x.test"}), &ctx())
            .unwrap_err(),
    );
    assert_eq!(error, "the request to https://x.test failed: dns");
}

#[test]
fn an_unknown_format_or_a_missing_url_is_refused_by_name() {
    let (tool, fetcher) = tool(MemoryFetcher::new());
    let error = message(
        tool.call(&json!({"url": "https://x.test", "format": "pdf"}), &ctx())
            .unwrap_err(),
    );
    assert!(
        error.contains("format must be markdown, text or html, not \"pdf\""),
        "{error}"
    );
    let error = message(tool.call(&json!({}), &ctx()).unwrap_err());
    assert_eq!(error, "missing string argument `url`");
    assert!(fetcher.asked().is_empty());
}

#[test]
fn a_plain_text_body_is_passed_through_untouched() {
    let body = "line one\nline two\n";
    let (tool, _) = tool(MemoryFetcher::new().answer("x.test", answer(200, "text/plain", body)));
    let out = text(
        tool.call(&json!({"url": "https://x.test"}), &ctx())
            .unwrap(),
    );
    assert!(out.ends_with(body), "{out}");
}
