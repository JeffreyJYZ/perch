//! The one blocking request and its error body.

use std::io::Read;
use std::sync::mpsc::Sender;

use crate::{Chunk, Error, Result};

use super::REQUEST_TIMEOUT;
use super::decode::read_events;

/// How much of an error body is kept before truncating it.
const ERROR_BODY_LIMIT: usize = 8 * 1024;

/// The provider thread: one request, then the decoded body, then either the final `Err` or a
/// closed channel.
pub(super) fn run(url: &str, api_key: &str, body: &[u8], tx: &Sender<Result<Chunk>>) {
    if let Err(error) = send_and_read(url, api_key, body, tx) {
        let _ = tx.send(Err(error));
    }
}

fn send_and_read(url: &str, api_key: &str, body: &[u8], tx: &Sender<Result<Chunk>>) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()?;
    let mut request = client
        .post(url)
        .header("accept", "text/event-stream")
        .header("content-type", "application/json")
        .body(body.to_vec());
    if !api_key.is_empty() {
        request = request.bearer_auth(api_key);
    }
    let mut response = request.send()?;
    let status = response.status();
    if !status.is_success() {
        let mut text = String::new();
        let _ = response.read_to_string(&mut text);
        return Err(Error::Status {
            status: status.as_u16(),
            body: truncate(text),
        });
    }
    read_events(&mut response, tx)
}

/// Keep the head of a provider error body; the tail helps nobody and an unbounded body is a
/// memory sink on a hostile server.
fn truncate(text: String) -> String {
    if text.len() <= ERROR_BODY_LIMIT {
        return text;
    }
    let mut cut = ERROR_BODY_LIMIT;
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}\n[truncated {} bytes]", &text[..cut], text.len() - cut)
}
