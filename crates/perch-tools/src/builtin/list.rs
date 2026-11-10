//! `list`: one directory, names only, directories marked with a trailing slash.

use std::path::Path;

use serde_json::{Value, json};

use super::{LIST_MAX_ENTRIES, object_schema};
use crate::{Error, Result, Tool, ToolCtx, ToolOutput, ToolSpec, confine};

pub struct List;

impl Tool for List {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "list".into(),
            description: "List the entries of a directory in the worktree.".into(),
            parameters: object_schema(
                json!({
                    "path": { "type": "string", "description": "Directory relative to the worktree root; defaults to `.`." }
                }),
                &[],
                "List one directory.",
            ),
        }
    }

    fn needs_approval(&self) -> bool {
        false
    }

    fn call(&self, args: &Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        let path = args.get("path").and_then(Value::as_str).unwrap_or(".");
        let target = confine(&ctx.root, Path::new(path))?;
        if !target.is_dir() {
            return Err(Error::Message(format!(
                "{} is not a directory",
                target.display()
            )));
        }
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&target)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_dir = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
            entries.push(if is_dir { format!("{name}/") } else { name });
        }
        entries.sort();
        let hidden = entries.len().saturating_sub(LIST_MAX_ENTRIES);
        entries.truncate(LIST_MAX_ENTRIES);
        let mut text = entries.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        if hidden > 0 {
            text.push_str(&format!("[truncated {hidden} entries]\n"));
        }
        Ok(ToolOutput::text(text))
    }
}
