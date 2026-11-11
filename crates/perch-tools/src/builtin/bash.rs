//! `bash`: `sh -c` in the root, both streams captured, killed after `timeout`.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};

use super::job;
use super::process::run_captured;
use super::text::truncate;
use super::{BASH_MAX_OUTPUT, BASH_TIMEOUT, command_arg, object_schema};
use crate::{Preview, Result, Tool, ToolCtx, ToolOutput, ToolSpec, confine};

pub struct Bash {
    timeout: Duration,
}

impl Bash {
    pub fn new() -> Self {
        Self {
            timeout: BASH_TIMEOUT,
        }
    }

    /// The same tool with a different deadline, so a test can prove the kill without waiting.
    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }
}

impl Default for Bash {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for Bash {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "bash".into(),
            description: format!(
                "Run a shell command with the worktree root as cwd. Killed after {} s; output \
                 capped at 16 KiB. With `background: true` the call returns at once with the job \
                 id, its pid and its log path, and the command keeps running until it exits or \
                 the `jobs` tool kills it.",
                self.timeout.as_secs()
            ),
            parameters: object_schema(
                json!({
                    "command": { "type": "string", "description": "Command passed to `sh -c`." },
                    "background": {
                        "type": "boolean",
                        "description": "Start the command as a job and return immediately instead of waiting. Read or kill it with the `jobs` tool."
                    }
                }),
                &["command"],
                "Run one shell command.",
            ),
        }
    }

    fn needs_approval(&self) -> bool {
        true
    }

    fn call(&self, args: &Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        let command = command_arg(args)?;
        if args
            .get("background")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return background(&command, ctx);
        }
        let cwd = confine(&ctx.root, Path::new("."))?;
        let mut child = Command::new("sh");
        child
            .arg("-c")
            .arg(&command)
            .current_dir(&cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let captured = run_captured(child, self.timeout)?;

        let mut text = String::new();
        push_block(&mut text, &captured.stdout);
        push_block(&mut text, &captured.stderr);
        if let Some(code) = captured.code.filter(|code| *code != 0) {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(&format!("exit {code}"));
        }
        Ok(ToolOutput::with_preview(
            truncate(text, BASH_MAX_OUTPUT),
            Preview::Command(command),
        ))
    }

    /// The command itself, which is the whole of what a shell call will do.
    fn preview(&self, args: &Value, ctx: &ToolCtx) -> Result<Option<Preview>> {
        let _ = ctx;
        Ok(Some(Preview::Command(command_arg(args)?)))
    }
}

/// Starts the command under a job and answers at once with what `jobs` can then act on.
fn background(command: &str, ctx: &ToolCtx) -> Result<ToolOutput> {
    let (id, pid, log) = job::start(command, &ctx.root)?;
    let answer = json!({
        "job": id,
        "command": command,
        "pid": pid,
        "log": log.display().to_string(),
    });
    let text = serde_json::to_string(&answer)
        .map_err(|error| crate::Error::Message(format!("cannot describe the job: {error}")))?;
    Ok(ToolOutput::with_preview(
        text,
        Preview::Command(command.to_string()),
    ))
}

fn push_block(text: &mut String, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&String::from_utf8_lossy(bytes));
}
