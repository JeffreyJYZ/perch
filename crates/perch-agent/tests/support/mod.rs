//! Shared test support: a local server that replays a recorded SSE body.
//!
//! Every file in `tests/` is its own crate, so this module is included rather than duplicated by
//! each test binary that needs the server.

#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// A fixture cannot ask the server to wait longer than this between events.
const MAX_PACE: u64 = 1_000;

/// One recorded fixture being served, and how much of it actually went out.
pub struct Fixture {
    pub base_url: String,
    /// Events written so far. A test uses it to prove its client stopped before the end.
    pub written: Arc<AtomicUsize>,
    /// Events the fixture holds, so "not finished" needs no magic number.
    pub total: usize,
}

/// Serve one recorded SSE body from `path` on `127.0.0.1:0` and return its base URL.
///
/// The connection is closed after the body, so a fixture that stops without `data: [DONE]`
/// looks exactly like a provider that cut the stream.
///
/// A fixture may pace itself with a directive comment as its first line, `: pace_ms 120`. The
/// server then writes one event, flushes, and waits that long before the next, which is what
/// lets a test cancel while the reply is still arriving. Without the directive the whole body is
/// written in one go, so a fixture that finishes instantly behaves exactly as before.
pub fn fixture_server(path: &str) -> String {
    serve(path).base_url
}

/// The same server as [`fixture_server`], also reporting how much of the fixture went out.
pub fn serve(path: &str) -> Fixture {
    let recorded = std::fs::read_to_string(path).unwrap();
    let (pause, body) = split_pace(&recorded);
    // Owned, because the serving thread outlives this stack frame.
    let body = body.to_string();
    let events: Vec<String> = body
        .split("\n\n")
        .filter(|event| !event.trim().is_empty())
        .map(str::to_string)
        .collect();
    let total = events.len();
    let written = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&written);
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
        if socket.write_all(head.as_bytes()).is_err() {
            return;
        }
        for (index, event) in events.iter().enumerate() {
            if index > 0
                && let Some(pause) = pause
            {
                std::thread::sleep(pause);
            }
            // A cancelled turn drops the connection, so a write failing is an expected outcome
            // here, not a test failure.
            if socket.write_all(event.as_bytes()).is_err() || socket.write_all(b"\n\n").is_err() {
                return;
            }
            if socket.flush().is_err() {
                return;
            }
            count.fetch_add(1, Ordering::SeqCst);
        }
        let _ = socket.shutdown(Shutdown::Write);
    });
    Fixture {
        base_url,
        written,
        total,
    }
}

/// Split an optional `: pace_ms <n>` directive off the head of a fixture.
fn split_pace(recorded: &str) -> (Option<Duration>, &str) {
    let Some((first, rest)) = recorded.split_once('\n') else {
        return (None, recorded);
    };
    let Some(millis) = first.trim().strip_prefix(": pace_ms ") else {
        return (None, recorded);
    };
    let millis = millis.trim().parse::<u64>().unwrap_or(0).min(MAX_PACE);
    (
        Some(Duration::from_millis(millis)),
        rest.strip_prefix('\n').unwrap_or(rest),
    )
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
