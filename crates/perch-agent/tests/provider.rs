//! Provider tests against a recorded SSE body served from `tests/fixtures`.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener};

use perch_agent::{ChatRequest, Chunk, Message, SecretString, stream};

/// Serve one recorded SSE body from `path` on `127.0.0.1:0`, returning its base URL.
///
/// The connection is closed after the body, so a fixture that stops without `data: [DONE]`
/// looks exactly like a provider that cut the stream.
fn fixture_server(path: &str) -> String {
    let body = std::fs::read(path).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        // A wrong path or body answers 400, so a regression in the request fails the test
        // instead of passing because the response was served regardless.
        let Some((line, request)) = read_request(&mut socket) else {
            return;
        };
        let path_and_body_are_right = line.contains(" /chat/completions ")
            && request.contains("\"stream\":true")
            && request.contains("\"include_usage\":true");
        if !path_and_body_are_right {
            let reason = format!("unexpected request: {line}\n{request}");
            let head = format!(
                "HTTP/1.1 400 Bad Request\r\ncontent-type: text/plain\r\n\
                 connection: close\r\ncontent-length: {}\r\n\r\n{reason}",
                reason.len()
            );
            let _ = socket.write_all(head.as_bytes());
            return;
        }
        let head = "HTTP/1.1 200 OK\r\n\
             content-type: text/event-stream\r\n\
             cache-control: no-cache\r\n\
             connection: close\r\n\r\n";
        socket.write_all(head.as_bytes()).unwrap();
        socket.write_all(&body).unwrap();
        socket.flush().unwrap();
        let _ = socket.shutdown(Shutdown::Write);
    });
    base_url
}

/// Read one request in full, as its request line plus body, so the client is never reset while
/// still writing.
fn read_request(socket: &mut std::net::TcpStream) -> Option<(String, String)> {
    let mut head = Vec::new();
    let mut buffer = [0u8; 4096];
    let end = loop {
        let read = socket.read(&mut buffer).unwrap();
        if read == 0 {
            return None;
        }
        head.extend_from_slice(&buffer[..read]);
        if let Some(end) = head.windows(4).position(|window| window == b"\r\n\r\n") {
            break end;
        }
    };
    let header = String::from_utf8_lossy(&head[..end]).to_string();
    let request_line = header.lines().next().unwrap_or_default().to_string();
    let content_length: usize = header
        .to_lowercase()
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0);
    let mut body = head[end + 4..].to_vec();
    while body.len() < content_length {
        let read = socket.read(&mut buffer).unwrap();
        if read == 0 {
            break;
        }
        body.extend_from_slice(&buffer[..read]);
    }
    Some((request_line, String::from_utf8_lossy(&body).into_owned()))
}

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
