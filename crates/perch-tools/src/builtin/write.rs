//! `write`: one file, parent directories created inside the root only.

use std::path::Path;

use serde_json::{Value, json};

use super::text::{line_count, unified};
use super::{object_schema, path_arg, string_arg};
use crate::{Preview, Result, Tool, ToolCtx, ToolOutput, ToolSpec, confine};

pub struct Write;

impl Tool for Write {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "write".into(),
            description: "Write a file, creating it and its parent directories if needed.".into(),
            parameters: object_schema(
                json!({
                    "path": { "type": "string", "description": "Path relative to the worktree root." },
                    "text": { "type": "string", "description": "The whole file content." }
                }),
                &["path", "text"],
                "Write one file.",
            ),
        }
    }

    fn needs_approval(&self) -> bool {
        true
    }

    fn call(&self, args: &Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        let target = confine(&ctx.root, Path::new(path_arg(args)?))?;
        let text = string_arg(args, "text")?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, &text)?;
        Ok(ToolOutput::text(format!(
            "wrote {} ({} bytes, {} lines)",
            target.display(),
            text.len(),
            line_count(&text)
        )))
    }

    /// The text as a diff: against the current file when one exists, as an addition when not.
    fn preview(&self, args: &Value, ctx: &ToolCtx) -> Result<Option<Preview>> {
        let path = path_arg(args)?;
        let target = confine(&ctx.root, Path::new(path))?;
        let text = string_arg(args, "text")?;
        // A file that is not UTF-8 is previewed lossily rather than refused: what the user
        // approves is the text that will replace it, and refusing would leave the call both
        // unapprovable and un-runnable.
        let before = match std::fs::read(&target) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error.into()),
        };
        Ok(Some(Preview::Diff(unified(path, &before, &text))))
    }
}
