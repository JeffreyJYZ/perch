//! `board`: read what this session has not seen, or post one entry.

use std::sync::Arc;

use perch_tools::{Result as ToolResult, Tool, ToolCtx, ToolOutput, ToolSpec};
use serde_json::{Value, json};

use crate::entry::Post;
use crate::error::Error;
use crate::store::Store;
use crate::tools::{files, optional, render};

/// Both halves live under one name because the model has one word for both: no argument reads,
/// `post` writes.
pub struct Board {
    store: Arc<Store>,
}

impl Board {
    pub(crate) fn new(store: Arc<Store>) -> Self {
        Self { store }
    }

    fn post(&self, args: &Value, text: &str) -> crate::Result<String> {
        if text.trim().is_empty() {
            return Err(Error::Message("`post` must not be empty".into()));
        }
        let post = Post {
            text: text.to_string(),
            to: optional(args, "to")?,
            group: optional(args, "group")?,
            files: files(args)?,
        };
        let entry = self.store.append(post)?;
        Ok(format!("posted #{}\n", entry.id))
    }
}

impl Tool for Board {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "board".into(),
            description:
                "Shared board for the sessions working in this project. Call with no arguments to \
				 read what this session has not seen; call with `post` to post an entry."
                    .into(),
            parameters: json!({
                "type": "object",
                "description": "Read this session's unseen entries, or post one.",
                "properties": {
                    "post": {
                        "type": "string",
                        "description": "The entry to append. Omit the whole argument to read instead."
                    },
                    "to": {
                        "type": "string",
                        "description": "Address the entry to one session; omit to address every session."
                    },
                    "group": {
                        "type": "string",
                        "description": "A label a lead can wait on, such as the wave being collected."
                    },
                    "files": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Paths this entry is about, relative to the project."
                    }
                },
                "required": [],
                "additionalProperties": false
            }),
        }
    }

    fn needs_approval(&self) -> bool {
        // The board is a message between sessions, not a change to the worktree, so there is
        // nothing for the approval view to show and nobody to interrupt for it.
        false
    }

    fn call(&self, args: &Value, _ctx: &ToolCtx) -> ToolResult<ToolOutput> {
        let text = match args.get("post") {
            None | Some(Value::Null) => None,
            Some(Value::String(text)) => Some(text),
            Some(_) => return Err(Error::Message("`post` must be a string".into()).into()),
        };
        let answer = match text {
            Some(text) => self.post(args, text),
            None => self.store.take().map(|read| render(&read)),
        };
        Ok(ToolOutput::text(answer?))
    }
}
