//! `edit`: replace `old` with `new` when `old` appears exactly once, returning a diff preview.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::text::unified;
use super::{object_schema, path_arg, string_arg};
use crate::{Error, Preview, Result, Tool, ToolCtx, ToolOutput, ToolSpec, confine};

pub struct Edit;

impl Tool for Edit {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "edit".into(),
            description:
                "Replace an exact snippet in a file. The old text must appear exactly once.".into(),
            parameters: object_schema(
                json!({
                    "path": { "type": "string", "description": "Path relative to the worktree root." },
                    "old": { "type": "string", "description": "Exact text to replace; must occur once." },
                    "new": { "type": "string", "description": "Replacement text." }
                }),
                &["path", "old", "new"],
                "Replace one exact snippet.",
            ),
        }
    }

    fn needs_approval(&self) -> bool {
        true
    }

    fn call(&self, args: &Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        let plan = plan_edit(args, ctx)?;
        std::fs::write(&plan.target, &plan.updated)?;
        Ok(ToolOutput::with_preview(
            format!("edited {}: 1 replacement", plan.path),
            Preview::Diff(plan.diff),
        ))
    }

    /// The same diff the call returns, produced without writing anything.
    fn preview(&self, args: &Value, ctx: &ToolCtx) -> Result<Option<Preview>> {
        Ok(Some(Preview::Diff(plan_edit(args, ctx)?.diff)))
    }
}

/// One `edit` call resolved and computed, with nothing written yet.
///
/// `call` and `preview` both go through here, so the diff a person approves is the diff the call
/// then returns; a second implementation of the replacement would let the two disagree.
struct EditPlan {
    path: String,
    target: PathBuf,
    updated: String,
    diff: String,
}

fn plan_edit(args: &Value, ctx: &ToolCtx) -> Result<EditPlan> {
    let path = path_arg(args)?;
    let target = confine(&ctx.root, Path::new(path))?;
    let old = string_arg(args, "old")?;
    let new = string_arg(args, "new")?;
    if old.is_empty() {
        return Err(Error::Message("`old` must not be empty".into()));
    }
    let original = std::fs::read_to_string(&target)?;
    let found = original.matches(&old).count();
    if found != 1 {
        return Err(Error::Message(format!(
            "`old` must appear exactly once in {path}, found {found} occurrences"
        )));
    }
    let updated = original.replacen(&old, &new, 1);
    let diff = unified(path, &original, &updated);
    Ok(EditPlan {
        path: path.to_string(),
        target,
        updated,
        diff,
    })
}
