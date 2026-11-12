//! `patch`: a unified diff across files, applied all-or-nothing.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::patch_apply::apply;
use super::patch_diff;
use super::text::unified;
use super::{object_schema, string_arg};
use crate::{Error, Preview, Result, Tool, ToolCtx, ToolOutput, ToolSpec, confine};

pub struct Patch;

impl Tool for Patch {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "patch".into(),
            description: "Apply a unified diff across one or more files in a single call. Every \
                          hunk of every file must match exactly; if any hunk fails, nothing is \
                          written and the error names the file and the hunk. Use `--- a/path`, \
                          `+++ b/path` and `@@ -l,c +l,c @@`; `/dev/null` on one side creates or \
                          deletes a file."
                .into(),
            parameters: object_schema(
                json!({
                    "diff": {
                        "type": "string",
                        "description": "A unified diff: `--- a/path`, `+++ b/path`, then `@@` hunks whose context lines are copied from the file."
                    }
                }),
                &["diff"],
                "Apply a unified diff to one or more files.",
            ),
        }
    }

    fn needs_approval(&self) -> bool {
        true
    }

    fn call(&self, args: &Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        let plan = plan_patch(args, ctx)?;
        for (target, text) in &plan.writes {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(target, text)?;
        }
        Ok(ToolOutput::with_preview(
            plan.summary,
            Preview::Diff(plan.diff),
        ))
    }

    /// The same diff the call would apply, computed without writing any of it.
    fn preview(&self, args: &Value, ctx: &ToolCtx) -> Result<Option<Preview>> {
        Ok(Some(Preview::Diff(plan_patch(args, ctx)?.diff)))
    }
}

/// Every file resolved and patched in memory, with nothing written yet.
///
/// `call` and `preview` both go through here, so what a person approves is exactly what the call
/// then writes, and a hunk that fails stops the whole diff before the first byte lands.
struct PatchPlan {
    writes: Vec<(PathBuf, String)>,
    diff: String,
    summary: String,
}

fn plan_patch(args: &Value, ctx: &ToolCtx) -> Result<PatchPlan> {
    let text = string_arg(args, "diff")?;
    let files = patch_diff::parse(&text)?;
    if files.is_empty() {
        return Err(Error::Message("the diff contains no file patches".into()));
    }

    let mut writes = Vec::new();
    let mut diff = String::new();
    let mut added = 0;
    let mut removed = 0;
    for file in &files {
        let path = file.path()?.to_string();
        let target = confine(&ctx.root, Path::new(&path))?;
        let before = match std::fs::read_to_string(&target) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error.into()),
        };
        let after = apply(&before, &file.hunks, &path)?;
        if after == before {
            continue;
        }
        added += file.added();
        removed += file.removed();
        diff.push_str(&unified(&path, &before, &after));
        writes.push((target, after));
    }
    if writes.is_empty() {
        return Err(Error::Message(
            "the diff changes nothing: every file already holds the patched text".into(),
        ));
    }

    let count = writes.len();
    let summary = format!(
        "{count} file{}, +{added} −{removed}",
        if count == 1 { "" } else { "s" }
    );
    Ok(PatchPlan {
        writes,
        diff,
        summary,
    })
}
