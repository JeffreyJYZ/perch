//! The event iterator the caller reads.

use std::sync::mpsc::Receiver;

use crate::{Chunk, Result};

/// The decoded events of one reply, in stream order.
///
/// The iterator yields exactly one `Err` when the reply fails, and that error is always its last
/// item: a half-received tool call can never be mistaken for a finished one by a caller that
/// stops at the first error.
pub struct ChunkStream {
    rx: Receiver<Result<Chunk>>,
    finished: bool,
}

impl ChunkStream {
    pub(super) fn new(rx: Receiver<Result<Chunk>>) -> Self {
        Self {
            rx,
            finished: false,
        }
    }
}

impl Iterator for ChunkStream {
    type Item = Result<Chunk>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        match self.rx.recv() {
            Ok(item) => {
                if item.is_err() {
                    self.finished = true;
                }
                Some(item)
            }
            Err(_) => {
                self.finished = true;
                None
            }
        }
    }
}
