//! Server-sent events in, [`Chunk`]s out.

use std::io::Read;
use std::sync::mpsc::Sender;

use crate::{Chunk, Error, Result};

use super::wire::WireChunk;

/// Read size for the response body.
const READ_BUFFER: usize = 8 * 1024;

/// Read the body, decode SSE events and forward them until `data: [DONE]`.
pub(super) fn read_events(response: &mut impl Read, tx: &Sender<Result<Chunk>>) -> Result<()> {
    let mut decoder = Decoder::default();
    let mut buffer = [0u8; READ_BUFFER];
    loop {
        let read = match response.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(Error::Io(error)),
        };
        for event in decoder.feed(&buffer[..read]) {
            if event == "[DONE]" {
                return Ok(());
            }
            dispatch(&event, tx)?;
        }
    }
    // The body ended and the provider never said it was done, so whatever arrived is partial.
    Err(Error::Truncated)
}

/// Split the body into events on blank lines, ignoring comments and every field but `data:`.
#[derive(Default)]
struct Decoder {
    pending: Vec<u8>,
    data: Vec<String>,
}

impl Decoder {
    /// Feed received bytes, returning every event completed by them.
    fn feed(&mut self, bytes: &[u8]) -> Vec<String> {
        self.pending.extend_from_slice(bytes);
        let mut events = Vec::new();
        while let Some(newline) = self.pending.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=newline).collect();
            let line = &line[..line.len() - 1];
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if line.is_empty() {
                if !self.data.is_empty() {
                    events.push(self.data.join("\n"));
                    self.data.clear();
                }
            } else if let Some(value) = line.strip_prefix(b"data:") {
                let value = value.strip_prefix(b" ").unwrap_or(value);
                self.data.push(String::from_utf8_lossy(value).into_owned());
            }
            // `:comment`, `event:`, `id:` and `retry:` carry nothing the decoder needs.
        }
        events
    }
}

/// Decode one event's data and forward the chunks it contains.
fn dispatch(data: &str, tx: &Sender<Result<Chunk>>) -> Result<()> {
    if data.trim().is_empty() {
        return Ok(());
    }
    let wire: WireChunk = serde_json::from_str(data)?;
    for choice in wire.choices {
        if let Some(content) = choice.delta.content
            && !content.is_empty()
        {
            send(tx, Chunk::Text(content))?;
        }
        for call in choice.delta.tool_calls.into_iter().flatten() {
            let function = call.function.unwrap_or_default();
            send(
                tx,
                Chunk::ToolCall {
                    index: call.index,
                    id: call.id,
                    name: function.name,
                    args_fragment: function.arguments.unwrap_or_default(),
                },
            )?;
        }
    }
    if let Some(usage) = wire.usage {
        send(
            tx,
            Chunk::Usage {
                input: usage.input(),
                output: usage.output(),
            },
        )?;
    }
    Ok(())
}

fn send(tx: &Sender<Result<Chunk>>, chunk: Chunk) -> Result<()> {
    tx.send(Ok(chunk)).map_err(|_| Error::Closed)
}
