//! Reading one request in full, and refusing one the fixture cannot serve.

use std::io::{Read, Write};
use std::net::TcpStream;

/// Answer a request the fixture cannot serve, so a wrong one fails the test instead of hanging.
pub fn refuse(socket: &mut TcpStream, reason: String) {
    let head = format!(
        "HTTP/1.1 400 Bad Request\r\ncontent-type: text/plain\r\n\
         connection: close\r\ncontent-length: {}\r\n\r\n{reason}",
        reason.len()
    );
    let _ = socket.write_all(head.as_bytes());
}

/// Read one request in full, as its request line plus body, so the client is never reset while
/// still writing.
pub fn read_request(socket: &mut TcpStream) -> Option<(String, String)> {
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
