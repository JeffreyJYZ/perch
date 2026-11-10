//! The server itself: one recorded body per request, written event by event.

use std::io::Write;
use std::net::{Shutdown, TcpListener};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use super::recording::{Recording, read_fixture};
use super::request::{read_request, refuse};

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
