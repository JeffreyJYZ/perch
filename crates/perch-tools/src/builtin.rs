//! The six shipped tools: every contract for paths, limits and output size lives here.

use std::fs::File;
use std::io::Read as IoRead;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use ignore::WalkBuilder;
use serde_json::{Value, json};
use similar::TextDiff;
use wait_timeout::ChildExt;

use crate::{Error, Preview, Result, Tool, ToolCtx, ToolOutput, ToolSpec, confine};

/// A shell call is killed after this long.
pub const BASH_TIMEOUT: Duration = Duration::from_secs(30);
/// A shell call never hands the model more than this many bytes.
pub const BASH_MAX_OUTPUT: usize = 16 * 1024;
/// `read` never reads more than this from disk.
pub const READ_MAX_BYTES: u64 = 64 * 1024;
/// `grep` never returns more than this many bytes.
pub const GREP_MAX_OUTPUT: usize = 16 * 1024;
/// `grep` stops after this many matching lines.
const GREP_MAX_MATCHES: usize = 200;
/// `list` stops after this many entries.
pub const LIST_MAX_ENTRIES: usize = 512;
/// Bytes kept from each child stream before the rest is drained and discarded.
const MAX_CAPTURE: usize = 1024 * 1024;
/// A file containing a NUL byte in its first block is treated as binary.
const BINARY_PROBE: u64 = 8 * 1024;

/// Every built-in tool, in the order the model sees them.
pub fn builtin_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(Read),
        Arc::new(Write),
        Arc::new(Edit),
        Arc::new(Bash::new()),
        Arc::new(List),
        Arc::new(Grep),
    ]
}

fn string_arg(args: &Value, key: &str) -> Result<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| Error::Message(format!("missing string argument `{key}`")))
}

fn path_arg(args: &Value) -> Result<&str> {
    args.get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Message("missing string argument `path`".to_string()))
}

fn object_schema(properties: Value, required: &[&str], what: &str) -> Value {
    json!({
        "type": "object",
        "description": what,
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

/// Cuts `text` to `budget` bytes, reporting how much was dropped.
fn truncate(text: String, budget: usize) -> String {
    if text.len() <= budget {
        return text;
    }
    let marker = format!("\n[truncated {} bytes]", text.len());
    let mut cut = budget.saturating_sub(marker.len());
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    let dropped = text.len() - cut;
    format!("{}\n[truncated {dropped} bytes]", &text[..cut])
}

fn line_count(text: &str) -> usize {
    text.lines().count()
}

/// `read`: one file, numbered lines, capped at `READ_MAX_BYTES`.
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

/// `write`: one file, parent directories created inside the root only.
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
}

/// `edit`: replace `old` with `new` when `old` appears exactly once, returning a diff preview.
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
        std::fs::write(&target, &updated)?;
        let diff = TextDiff::from_lines(&original, &updated)
            .unified_diff()
            .context_radius(3)
            .header(path, path)
            .to_string();
        Ok(ToolOutput::with_preview(
            format!("edited {path}: 1 replacement"),
            Preview::Diff(diff),
        ))
    }
}

/// `bash`: `sh -c` in the root, both streams captured, killed after `timeout`.
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
                "Run a shell command with the worktree root as cwd. Killed after {} s; output capped at 16 KiB.",
                self.timeout.as_secs()
            ),
            parameters: object_schema(
                json!({
                    "command": { "type": "string", "description": "Command passed to `sh -c`." }
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
        let command = string_arg(args, "command")?;
        if command.trim().is_empty() {
            return Err(Error::Message("`command` must not be empty".into()));
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

struct Captured {
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// Runs a child under a deadline, draining both pipes so a chatty command cannot deadlock.
fn run_captured(mut cmd: Command, timeout: Duration) -> Result<Captured> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    let mut child = cmd
        .spawn()
        .map_err(|e| Error::Message(format!("cannot run {program}: {e}")))?;
    let out_pipe = child.stdout.take();
    let err_pipe = child.stderr.take();

    std::thread::scope(|scope| {
        let out_reader = out_pipe.map(|pipe| scope.spawn(move || drain(pipe)));
        let err_reader = err_pipe.map(|pipe| scope.spawn(move || drain(pipe)));

        let status = match child.wait_timeout(timeout) {
            Ok(Some(status)) => Ok(status),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(Error::Timeout {
                    program: program.clone(),
                    after: timeout,
                })
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(Error::Io(e))
            }
        };

        let stdout = out_reader.and_then(|h| h.join().ok()).unwrap_or_default();
        let stderr = err_reader.and_then(|h| h.join().ok()).unwrap_or_default();
        match status {
            Ok(status) => Ok(Captured {
                code: status.code(),
                stdout,
                stderr,
            }),
            Err(e) => Err(e),
        }
    })
}

/// Reads a pipe to the end, keeping the first `MAX_CAPTURE` bytes so the child never blocks.
fn drain(mut pipe: impl IoRead) -> Vec<u8> {
    let mut kept = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        match pipe.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if kept.len() < MAX_CAPTURE {
                    let room = MAX_CAPTURE - kept.len();
                    kept.extend_from_slice(&buf[..n.min(room)]);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    kept
}

/// `list`: one directory, names only, directories marked with a trailing slash.
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

/// `grep`: literal substring search, skipping binary files and honouring ignore rules.
pub struct Grep;

impl Tool for Grep {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "grep".into(),
            description:
                "Search a file or directory for a literal string. Binary files are skipped.".into(),
            parameters: object_schema(
                json!({
                    "pattern": { "type": "string", "description": "Literal text to find." },
                    "path": { "type": "string", "description": "File or directory relative to the worktree root; defaults to `.`." }
                }),
                &["pattern"],
                "Search for a literal string.",
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

        let mut matches = Vec::new();
        for entry in WalkBuilder::new(&base)
            .build()
            .filter_map(std::result::Result::ok)
        {
            if matches.len() >= GREP_MAX_MATCHES {
                break;
            }
            if !entry
                .file_type()
                .map(|kind| kind.is_file())
                .unwrap_or(false)
            {
                continue;
            }
            let Some(body) = read_text(entry.path()) else {
                continue;
            };
            let shown = entry.path().strip_prefix(&root).unwrap_or(entry.path());
            for (index, line) in body.lines().enumerate() {
                if line.contains(&pattern) {
                    matches.push(format!("{}:{}: {}", shown.display(), index + 1, line));
                    if matches.len() >= GREP_MAX_MATCHES {
                        break;
                    }
                }
            }
        }
        matches.sort();
        let mut text = matches.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        Ok(ToolOutput::text(truncate(text, GREP_MAX_OUTPUT)))
    }
}

/// Reads a text file, or answers `None` for a binary or unreadable one.
fn read_text(path: &Path) -> Option<String> {
    let mut head = Vec::new();
    File::open(path)
        .ok()?
        .take(BINARY_PROBE)
        .read_to_end(&mut head)
        .ok()?;
    if head.contains(&0) {
        return None;
    }
    std::fs::read_to_string(path).ok()
}
