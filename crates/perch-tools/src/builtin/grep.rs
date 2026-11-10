//! `grep`: ripgrep, honouring ignore files and skipping binary files.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};

use super::process::run_captured;
use super::text::truncate;
use super::{BASH_TIMEOUT, GREP_MAX_OUTPUT, object_schema, string_arg};
use crate::{Error, Result, Tool, ToolCtx, ToolOutput, ToolSpec, confine};

pub struct Grep {
    program: PathBuf,
    timeout: Duration,
}

impl Grep {
    pub fn new() -> Self {
        Self {
            program: PathBuf::from("rg"),
            timeout: BASH_TIMEOUT,
        }
    }

    /// The same tool with a different binary, so a test can prove a missing one is an error.
    pub fn with_program(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            timeout: BASH_TIMEOUT,
        }
    }
}

impl Default for Grep {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for Grep {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "grep".into(),
            description:
                "Search a file or directory with a regular expression, honouring .gitignore and skipping binary files."
                    .into(),
            parameters: object_schema(
                json!({
                    "pattern": { "type": "string", "description": "Regular expression, in ripgrep syntax." },
                    "path": { "type": "string", "description": "File or directory relative to the worktree root; defaults to `.`." }
                }),
                &["pattern"],
                "Search for a regular expression.",
            ),
        }
    }

    fn needs_approval(&self) -> bool {
        false
    }

    fn call(&self, args: &Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        let pattern = string_arg(args, "pattern")?;
        if pattern.is_empty() {
            return Err(Error::Message("`pattern` must not be empty".into()));
        }
        let path = args.get("path").and_then(Value::as_str).unwrap_or(".");
        let root = ctx.root.canonicalize()?;
        let base = confine(&ctx.root, Path::new(path))?;
        let relative = base.strip_prefix(&root).unwrap_or(Path::new("."));
        let relative = if relative.as_os_str().is_empty() {
            Path::new(".")
        } else {
            relative
        };

        let mut child = Command::new(self.program.as_os_str());
        child
            .arg("--line-number")
            .arg("--no-heading")
            .arg("--color=never")
            .arg("--with-filename")
            .arg("--no-config")
            // ripgrep only honours .gitignore inside a git repository by default, and a
            // worktree root is not always one; the ignore rule must not depend on that.
            .arg("--no-require-git")
            .arg("--")
            .arg(&pattern)
            .arg(relative)
            .current_dir(&root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let captured = run_captured(child, self.timeout)?;
        let stderr = String::from_utf8_lossy(&captured.stderr).trim().to_string();
        match captured.code {
            Some(0) => Ok(ToolOutput::text(truncate(
                strip_dot_slash(&String::from_utf8_lossy(&captured.stdout)),
                GREP_MAX_OUTPUT,
            ))),
            // ripgrep exits 1 for "no matches", which is an empty result, not a failure.
            Some(1) if stderr.is_empty() => Ok(ToolOutput::text(String::new())),
            _ => Err(Error::Message(if stderr.is_empty() {
                format!(
                    "{} exited with {}",
                    self.program.display(),
                    captured
                        .code
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "a signal".to_string())
                )
            } else {
                stderr
            })),
        }
    }
}

/// ripgrep prefixes hits with `./` when the search root is `.`; paths should read the way the
/// model would write them, relative to the root with no prefix.
fn strip_dot_slash(stdout: &str) -> String {
    let mut text = String::with_capacity(stdout.len());
    for line in stdout.lines() {
        text.push_str(line.strip_prefix("./").unwrap_or(line));
        text.push('\n');
    }
    text
}
