//! `websearch` end to end, answered from a captured result page so no test touches the network.

use std::path::PathBuf;
use std::sync::Arc;

use perch_tools::{Error, Tool, ToolCtx};
use serde_json::json;

use super::{SEARCH_COUNT_DEFAULT, SEARCH_COUNT_MAX, WebSearch, parse};
use crate::net::{MemoryFetcher, Response};

fn ctx() -> ToolCtx {
    ToolCtx {
        root: PathBuf::from("/"),
    }
}

/// One result block in the shape DuckDuckGo serves: a `result__a` anchor around the title and a
/// `result__snippet` anchor around the snippet, with the URL behind a `uddg=` redirect.
fn block(url: &str, title: &str, snippet: &str) -> String {
    let escaped = super::encode::query(url);
    format!(
        r#"<div class="result"><h2><a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg={escaped}&amp;rut=9"> {title} </a></h2><a class="result__snippet" href="{url}"> {snippet} </a></div>"#
    )
}

fn page(count: usize) -> String {
    let mut page = String::new();
    for index in 0..count {
        let url = format!("https://example.test/{index}");
        page.push_str(&block(
            &url,
            &format!("Title <b>{index}</b>"),
            "Snippet\n   over two lines.",
        ));
    }
    page
}

fn response(status: u16, body: &str) -> Response {
    Response {
        status,
        url: "https://html.duckduckgo.com/html/?q=x".into(),
        content_type: "text/html; charset=utf-8".into(),
        body: body.as_bytes().to_vec(),
    }
}

fn tool(fetcher: MemoryFetcher) -> (WebSearch, Arc<MemoryFetcher>) {
    let fetcher = Arc::new(fetcher);
    (WebSearch::with_fetcher(fetcher.clone()), fetcher)
}

fn message(error: Error) -> String {
    match error {
        Error::Message(words) => words,
        other => panic!("expected a plain message, got {other:?}"),
    }
}

fn call(tool: &WebSearch, args: serde_json::Value) -> String {
    tool.call(&args, &ctx()).expect("a parsed page").text
}

#[test]
fn a_result_carries_its_title_url_and_one_line_snippet() {
    let (tool, _) = tool(MemoryFetcher::new().answer("duckduckgo", response(200, &page(2))));
    let out = call(&tool, json!({"query": "rune"}));
    assert!(out.contains("1. Title 0"), "{out}");
    assert!(out.contains("   https://example.test/0"), "{out}");
    assert!(out.contains("   Snippet over two lines."), "{out}");
    assert!(out.contains("2. Title 1"), "{out}");
    assert!(
        !out.contains("uddg="),
        "the redirect was not unwrapped:\n{out}"
    );
}

#[test]
fn the_same_url_twice_is_reported_once_with_the_first_snippet() {
    let page = block("https://example.test/same", "First", "best")
        + &block("https://example.test/other", "Second", "second")
        + &block("https://example.test/same", "Again", "worse");
    let hits = parse::dedupe(parse::results(&page));
    assert_eq!(hits.len(), 2, "{hits:?}");
    assert_eq!(hits[0].title, "First");
    assert_eq!(hits[0].snippet, "best");
}

#[test]
fn the_default_count_is_eight_and_the_most_is_twenty() {
    assert_eq!(SEARCH_COUNT_DEFAULT, 8);
    assert_eq!(SEARCH_COUNT_MAX, 20);
    let (eight, _) = tool(MemoryFetcher::new().answer("duckduckgo", response(200, &page(25))));
    let out = call(&eight, json!({"query": "rune"}));
    assert_eq!(out.matches("https://example.test/").count(), 8, "{out}");

    let (many, _) = tool(MemoryFetcher::new().answer("duckduckgo", response(200, &page(25))));
    let out = call(&many, json!({"query": "rune", "count": 100}));
    assert_eq!(out.matches("https://example.test/").count(), 20, "{out}");

    let (one, _) = tool(MemoryFetcher::new().answer("duckduckgo", response(200, &page(3))));
    let out = call(&one, json!({"query": "rune", "count": 0}));
    assert_eq!(out.matches("https://example.test/").count(), 1, "{out}");
}

#[test]
fn a_challenge_page_is_quoted_rather_than_reported_as_no_results() {
    let body = "<html><body>If this persists, please let us know.</body></html>";
    let (tool, _) = tool(MemoryFetcher::new().answer("duckduckgo", response(202, body)));
    let error = message(tool.call(&json!({"query": "rune"}), &ctx()).unwrap_err());
    assert!(error.contains("answered HTTP 202"), "{error}");
    assert!(
        error.contains("If this persists, please let us know."),
        "{error}"
    );
    assert!(error.contains("q=rune"), "{error}");
}

#[test]
fn a_2xx_page_with_no_parseable_result_still_quotes_what_arrived() {
    let (tool, _) = tool(MemoryFetcher::new().answer(
        "duckduckgo",
        response(200, "<html><body>No results.</body></html>"),
    ));
    let error = message(tool.call(&json!({"query": "zzzz"}), &ctx()).unwrap_err());
    assert!(error.contains("no parseable results"), "{error}");
    assert!(error.contains("No results."), "{error}");
}

#[test]
fn a_non_2xx_answer_names_the_status_and_the_body() {
    let (tool, _) = tool(MemoryFetcher::new().answer("duckduckgo", response(503, "upstream down")));
    let error = message(tool.call(&json!({"query": "rune"}), &ctx()).unwrap_err());
    assert!(error.contains("HTTP 503"), "{error}");
    assert!(error.contains("upstream down"), "{error}");
}

#[test]
fn the_query_is_percent_encoded_into_the_endpoint_it_names() {
    let (tool, fetcher) = tool(MemoryFetcher::new().answer("duckduckgo", response(200, &page(1))));
    call(&tool, json!({"query": "rust & rune"}));
    let asked = fetcher.asked();
    assert_eq!(asked.len(), 1);
    assert_eq!(
        asked[0],
        "https://html.duckduckgo.com/html/?q=rust+%26+rune"
    );
}

#[test]
fn an_empty_query_is_refused_before_any_request() {
    let (tool, fetcher) = tool(MemoryFetcher::new());
    let error = message(tool.call(&json!({"query": "   "}), &ctx()).unwrap_err());
    assert_eq!(error, "missing string argument `query`");
    assert!(fetcher.asked().is_empty());
}

#[test]
fn quote_keeps_two_hundred_characters_and_one_line() {
    let long = "word\n".repeat(100);
    let quoted = super::quote(&long);
    assert_eq!(quoted.len(), 200);
    assert!(!quoted.contains('\n'));
}

#[test]
fn both_tools_are_offered_and_none_needs_approval() {
    let names: Vec<String> = crate::tools().iter().map(|tool| tool.spec().name).collect();
    assert_eq!(names, ["webfetch", "websearch"]);
    assert!(crate::tools().iter().all(|tool| !tool.needs_approval()));
}
