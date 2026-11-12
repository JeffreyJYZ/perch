//! `jobs`: list the background jobs this session started, read one's log, or kill one.

use serde_json::{Value, json};

use super::job::{self, JOB_MAX_CONCURRENT};
use super::object_schema;
use crate::{Error, Result, Tool, ToolCtx, ToolOutput, ToolSpec};

/// `jobs read` shows this many trailing lines unless asked for another count.
const DEFAULT_LINES: usize = 40;
/// The most lines one `jobs read` will ever return.
const MAX_LINES: usize = 200;

pub struct Jobs;

impl Tool for Jobs {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "jobs".into(),
            description: format!(
                "Background jobs started with `bash` and `background: true`. \
                 `op: \"list\"` shows every job this session started: id, command, state \
                 (running/exited/killed), exit code, elapsed and log path. \
                 `op: \"read\"` returns the tail of one job's log; `id` names the job and \
                 `lines` how many trailing lines to show (default {DEFAULT_LINES}, at most \
                 {MAX_LINES}); the answer says when it truncated. \
                 `op: \"kill\"` kills one job's whole process group by `id`, and a job that \
                 already stopped is reported rather than refused. At most {JOB_MAX_CONCURRENT} \
                 jobs run at once in one worktree; a finished job stays readable."
            ),
            parameters: object_schema(
                json!({
                    "op": {
                        "type": "string",
                        "enum": ["list", "read", "kill"],
                        "description": "The operation: `list` every job, `read` one log, `kill` one job's process group."
                    },
                    "id": {
                        "type": "string",
                        "description": "Job id such as `j1`. Required for `read` and `kill`."
                    },
                    "lines": {
                        "type": "integer",
                        "description": format!(
                            "For `read`: how many trailing log lines, default {DEFAULT_LINES}, capped at {MAX_LINES}."
                        )
                    }
                }),
                &["op"],
                "Inspect or kill a background job.",
            ),
        }
    }

    fn needs_approval(&self) -> bool {
        false
    }

    fn call(&self, args: &Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        let op = args.get("op").and_then(Value::as_str).ok_or_else(|| {
            Error::Message("missing string argument `op` (list, read, kill)".into())
        })?;
        match op {
            "list" => Ok(ToolOutput::text(list_text(ctx)?)),
            "read" => {
                let id = job_id(args)?;
                let lines = args
                    .get("lines")
                    .and_then(Value::as_u64)
                    .map(|count| count as usize)
                    .unwrap_or(DEFAULT_LINES)
                    .clamp(1, MAX_LINES);
                Ok(ToolOutput::text(job::read(&ctx.root, &id, lines)?))
            }
            "kill" => Ok(ToolOutput::text(job::kill(&ctx.root, &job_id(args)?)?)),
            other => Err(Error::Message(format!(
                "unknown `op` `{other}`; use `list`, `read` or `kill`"
            ))),
        }
    }
}

fn job_id(args: &Value) -> Result<String> {
    args.get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| Error::Message("missing string argument `id`".into()))
}

fn list_text(ctx: &ToolCtx) -> Result<String> {
    let views = job::list(&ctx.root)?;
    if views.is_empty() {
        return Ok("no background jobs".to_string());
    }
    let mut text = String::new();
    for view in views {
        text.push_str(&format!(
            "{} {} pid {} {:.1}s {}  log {}\n",
            view.id,
            view.state,
            view.pid,
            view.elapsed.as_secs_f64(),
            one_line(&view.command),
            view.log.display()
        ));
        if let Some(code) = view.code {
            text.push_str(&format!("  exit code {code}\n"));
        }
    }
    Ok(text)
}

/// A command with its newlines folded, so one job stays one line in the list.
fn one_line(command: &str) -> String {
    let folded = command.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut short: String = folded.chars().take(80).collect();
    if folded.chars().count() > 80 {
        short.push('…');
    }
    short
}
