//! `read`: one file, numbered lines, capped at `READ_MAX_BYTES`.

use std::fs::File;
use std::io::Read as IoRead;
use std::path::Path;

use serde_json::{Value, json};

use super::{READ_MAX_BYTES, object_schema, path_arg};
use crate::{Error, Result, Tool, ToolCtx, ToolOutput, ToolSpec, confine};

pub struct Read;

impl Tool for Read {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "read".into(),
            description: "Read a file from the worktree and return its lines with line numbers."
                .into(),
            parameters: object_schema(
                json!({
                    "path": { "type": "string", "description": "Path relative to the worktree root." }
                }),
                &["path"],
                "Read one file.",
            ),
        }
    }

    fn needs_approval(&self) -> bool {
        false
    }

    fn call(&self, args: &Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        let target = confine(&ctx.root, Path::new(path_arg(args)?))?;
        let file = File::open(&target)?;
        if !file.metadata()?.is_file() {
            return Err(Error::Message(format!(
                "{} is not a file",
                target.display()
            )));
        }
        let mut bytes = Vec::new();
        file.take(READ_MAX_BYTES + 1).read_to_end(&mut bytes)?;
        let truncated = bytes.len() as u64 > READ_MAX_BYTES;
        bytes.truncate(READ_MAX_BYTES as usize);

        let body = String::from_utf8_lossy(&bytes);
        let mut text = String::new();
        for (index, line) in body.lines().enumerate() {
            text.push_str(&format!("{:>6}\t{line}\n", index + 1));
        }
        if truncated {
            text.push_str(&format!("\n[truncated at {READ_MAX_BYTES} bytes]"));
        }
        Ok(ToolOutput::text(text))
    }
}
