//! Shared test support: a local server that replays recorded SSE bodies.
//!
//! Every file in `tests/` is its own crate, so this module is included rather than duplicated by
//! each test binary that needs the server.
//!
//! One fixture holds one response per request, in order. A turn that calls a tool asks again with
//! the result, so a recording of an agentic turn carries more than one response: `: next request`
//! on a line of its own starts the next. The whole body of every request is kept, which is how a
//! test reads the messages it was sent rather than trusting the shape of a struct.

#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A fixture cannot ask the server to wait longer than this between events.
const MAX_PACE: u64 = 1_000;

/// One recorded fixture being served, and what it actually did.
pub struct Fixture {
    pub base_url: String,
    /// Events written so far. A test uses it to prove its client stopped before the end.
    pub written: Arc<AtomicUsize>,
    /// Events the fixture's first response holds, so "not finished" needs no magic number.
    pub total: usize,
    /// How many requests arrived, so a test can prove the turn asked a second time.
    pub requests: Arc<AtomicUsize>,
    /// The body of every request, in arrival order.
    pub bodies: Arc<Mutex<Vec<String>>>,
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
    let Recording {
        pause,
        responses,
        repeat,
    } = read_fixture(path);
    let total = responses.first().map_or(0, Vec::len);
    let written = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&written);
    let requests = Arc::new(AtomicUsize::new(0));
    let served = Arc::clone(&requests);
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let received = Arc::clone(&bodies);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        loop {
            let Ok((mut socket, _)) = listener.accept() else {
                return;
            };
            // A wrong path, body or count answers 400, so a regression in the request fails the
            // test instead of passing because the response was served regardless.
            let Some((line, request)) = read_request(&mut socket) else {
                continue;
            };
            let index = served.fetch_add(1, Ordering::SeqCst);
            received.lock().unwrap().push(request.clone());
            let expected = responses
                .get(index)
                .or_else(|| repeat.then(|| responses.last()).flatten());
            let Some(events) = expected else {
                refuse(
                    &mut socket,
                    format!(
                        "unexpected request {}: the fixture holds {} response(s)",
                        index + 1,
                        responses.len()
                    ),
                );
                continue;
            };
            let path_and_body_are_right = line.contains(" /chat/completions ")
                && request.contains("\"stream\":true")
                && request.contains("\"include_usage\":true");
            if !path_and_body_are_right {
                refuse(
                    &mut socket,
                    format!("unexpected request: {line}\n{request}"),
                );
                continue;
            }
            let head = "HTTP/1.1 200 OK\r\n\
                 content-type: text/event-stream\r\n\
                 cache-control: no-cache\r\n\
                 connection: close\r\n\r\n";
            if socket.write_all(head.as_bytes()).is_err() {
                continue;
            }
            for (position, event) in events.iter().enumerate() {
                if position > 0
                    && let Some(pause) = pause
                {
                    std::thread::sleep(pause);
                }
                // A cancelled turn drops the connection, so a write failing is an expected
                // outcome here, not a test failure.
                if socket.write_all(event.as_bytes()).is_err() || socket.write_all(b"\n\n").is_err()
                {
                    break;
                }
                if socket.flush().is_err() {
                    break;
                }
                count.fetch_add(1, Ordering::SeqCst);
            }
            let _ = socket.shutdown(Shutdown::Write);
        }
    });
    Fixture {
        base_url,
        written,
        total,
        requests,
        bodies,
    }
}

/// Answer a request the fixture cannot serve, so a wrong one fails the test instead of hanging.
fn refuse(socket: &mut std::net::TcpStream, reason: String) {
    let head = format!(
        "HTTP/1.1 400 Bad Request\r\ncontent-type: text/plain\r\n\
         connection: close\r\ncontent-length: {}\r\n\r\n{reason}",
        reason.len()
    );
    let _ = socket.write_all(head.as_bytes());
}

/// One recorded fixture: the responses it holds and how it wants to be served.
struct Recording {
    /// How long to wait between two events of a response, when the fixture says.
    pause: Option<Duration>,
    /// One entry per request: the SSE events of that response.
    responses: Vec<Vec<String>>,
    /// Whether the last response answers every further request, for a turn that never settles.
    repeat: bool,
}

/// Read a recording into its responses.
fn read_fixture(path: &str) -> Recording {
    let recorded = std::fs::read_to_string(path).unwrap();
    let mut pause = None;
    let mut repeat = false;
    let mut bodies: Vec<String> = vec![String::new()];
    for line in recorded.lines() {
        let Some(note) = line.trim_start().strip_prefix(": ") else {
            if let Some(body) = bodies.last_mut() {
                body.push_str(line);
                body.push('\n');
            }
            continue;
        };
        let note = note.trim();
        if let Some(millis) = note.strip_prefix("pace_ms ") {
            let millis = millis.trim().parse::<u64>().unwrap_or(0).min(MAX_PACE);
            pause = Some(Duration::from_millis(millis));
        } else if note == "next request" {
            bodies.push(String::new());
        } else if note == "repeat" {
            repeat = true;
        }
        // Any other `: ...` line is a note for whoever recorded it, not something to send.
    }
    Recording {
        pause,
        responses: bodies.iter().map(|body| events_of(body)).collect(),
        repeat,
    }
}

/// The SSE events of one response, each kept with its own `data:` lines.
fn events_of(body: &str) -> Vec<String> {
    body.split("\n\n")
        .filter(|event| !event.trim().is_empty())
        .map(str::to_string)
        .collect()
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
