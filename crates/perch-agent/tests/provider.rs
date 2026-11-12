//! Provider tests against a recorded SSE body served from `tests/fixtures`.

mod support;

use perch_agent::{ChatRequest, Chunk, Message, SecretString, stream};

use support::fixture_server;

/// A request pointed at `base_url`, with one user message and the tool a fixture calls.
fn request_against(base_url: &str) -> ChatRequest {
    ChatRequest {
        base_url: base_url.to_string(),
        api_key: SecretString::from("test-key"),
        model: "test/model".to_string(),
        messages: vec![Message::user("go")],
        tools: vec![],
    }
}

#[test]
fn parses_text_tool_calls_and_usage_from_recorded_sse() {
    let server = fixture_server("tests/fixtures/sse_simple.txt");
    let chunks = stream(&request_against(&server)).unwrap();
    let mut text = String::new();
    let mut args = String::new();
    let mut usage = None;
    for chunk in chunks {
        match chunk.unwrap() {
            Chunk::Text(t) => text.push_str(&t),
            Chunk::ToolCall {
                args_fragment: fragment,
                ..
            } => args.push_str(&fragment),
            Chunk::Usage { input, output } => usage = Some((input, output)),
            // Thinking is its own stream: the answer is what this test is about.
            Chunk::Reasoning(_) => {}
        }
    }
    assert_eq!(text, "done");
    assert_eq!(args, "{\"path\":\"src/main.rs\"}");
    assert_eq!(usage, Some((12, 3)));
}

#[test]
fn a_cut_stream_ends_with_an_error_not_a_partial_tool_call() {
    let server = fixture_server("tests/fixtures/sse_cut.txt");
    let mut chunks = stream(&request_against(&server)).unwrap();
    let last = chunks.by_ref().last().unwrap();
    assert!(last.is_err());
}
