//! `board_wait`: park until this session's matching entry arrives, then read it.

use std::sync::Arc;
use std::time::{Duration, Instant};

use perch_tools::{Result as ToolResult, Tool, ToolCtx, ToolOutput, ToolSpec};
use serde_json::{Value, json};

use crate::cancel::Stopped;
use crate::error::Error;
use crate::store::Store;
use crate::tools::render;

/// How long a wait parks when the model does not say.
pub const WAIT_DEFAULT_MS: u64 = 30_000;
/// The longest a wait may park, whatever the model asks for.
pub const WAIT_MAX_MS: u64 = 30_000;

/// How often the log is re-read and the turn's cancellation checked while parked.
const POLL: Duration = Duration::from_millis(25);

pub struct BoardWait {
    store: Arc<Store>,
    stopped: Arc<dyn Stopped>,
}

impl BoardWait {
    pub(crate) fn new(store: Arc<Store>, stopped: Arc<dyn Stopped>) -> Self {
        Self { store, stopped }
    }

    /// Parks until a match, then reads; on the deadline it reads anyway and says it timed out.
    ///
    /// The deadline is a cap and not a promise: a cancellation ends the wait at the next poll, so
    /// a stopped turn never leaves this call parked for the full timeout.
    fn park(&self, needle: &str, timeout: Duration) -> crate::Result<String> {
        let deadline = Instant::now() + timeout;
        let mut offset = 0;
        loop {
            if self.stopped.stopped() {
                return Err(Error::Message(
                    "the turn was cancelled while waiting".into(),
                ));
            }
            let (found, next) = self.store.peek(offset, needle)?;
            offset = next;
            if found {
                return self.store.take().map(|read| render(&read));
            }
            let now = Instant::now();
            if now >= deadline {
                let read = self.store.take()?;
                let text = render(&read);
                return Ok(format!("{text}timed out after {}ms\n", timeout.as_millis()));
            }
            std::thread::sleep(POLL.min(deadline - now));
        }
    }
}

impl Tool for BoardWait {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "board_wait".into(),
            description:
                "Block until a matching board entry arrives for this session, then return \
			              it. `match` is a case-sensitive substring, `timeout_ms` caps the wait."
                    .into(),
            parameters: json!({
                "type": "object",
                "description": "Park on the board until an entry this session can read matches.",
                "properties": {
                    "match": {
                        "type": "string",
                        "description": "Case-sensitive substring; an entry matches when its text, \
                                        group, sender or one of its file paths contains it."
                    },
                    "timeout_ms": {
                        "type": "integer",
                        "minimum": 0,
                        "description": format!(
                            "How long to park. Defaults to {WAIT_DEFAULT_MS} and never exceeds {WAIT_MAX_MS}."
                        )
                    }
                },
                "required": ["match"],
                "additionalProperties": false
            }),
        }
    }

    fn needs_approval(&self) -> bool {
        false
    }

    fn call(&self, args: &Value, _ctx: &ToolCtx) -> ToolResult<ToolOutput> {
        let needle = args
            .get("match")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::Message("missing string argument `match`".into()))?;
        let asked = args
            .get("timeout_ms")
            .and_then(|value| {
                value
                    .as_u64()
                    .or_else(|| value.as_f64().map(|ms| ms.max(0.0) as u64))
            })
            .unwrap_or(WAIT_DEFAULT_MS);
        let timeout = Duration::from_millis(asked.min(WAIT_MAX_MS));
        Ok(ToolOutput::text(self.park(needle, timeout)?))
    }
}
