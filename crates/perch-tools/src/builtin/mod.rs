//! The shipped tools: every contract for paths, limits and output size lives here.
//!
//! Each tool is its own module, and the pieces they share are too: [`text`] holds the diff and
//! truncation helpers, [`process`] runs a child under a deadline, and the argument helpers below
//! are the one place an argument's shape is read.

mod bash;
mod edit;
mod glob;
mod grep;
mod job;
mod jobs;
mod list;
mod patch;
mod patch_apply;
mod patch_diff;
mod patch_hunk;
mod process;
mod read;
mod text;
mod todo;
mod todo_read;
mod todo_write;
mod write;

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use crate::{Error, Result, Tool};

pub use bash::Bash;
pub use edit::Edit;
pub use glob::Glob;
pub use grep::Grep;
pub use job::JOB_MAX_CONCURRENT;
pub use jobs::Jobs;
pub use list::List;
pub use patch::Patch;
pub use read::Read;
pub use todo::{MAX_TODOS, Status as TodoStatus, Todo, todos};
pub use todo_read::TodoRead;
pub use todo_write::TodoWrite;
pub use write::Write;

/// A shell call is killed after this long.
pub const BASH_TIMEOUT: Duration = Duration::from_secs(30);
/// A shell call never hands the model more than this many bytes.
pub const BASH_MAX_OUTPUT: usize = 16 * 1024;
/// `read` never reads more than this from disk.
pub const READ_MAX_BYTES: u64 = 64 * 1024;
/// `grep` never returns more than this many bytes.
pub const GREP_MAX_OUTPUT: usize = 16 * 1024;
/// `list` stops after this many entries.
pub const LIST_MAX_ENTRIES: usize = 512;

/// Every built-in tool, in the order the model sees them.
pub fn builtin_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(Read),
        Arc::new(Write),
        Arc::new(Edit),
        Arc::new(Bash::new()),
        Arc::new(Jobs),
        Arc::new(List),
        Arc::new(Grep::new()),
        Arc::new(Glob),
        Arc::new(Patch),
        Arc::new(TodoWrite),
        Arc::new(TodoRead),
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

/// The command a `bash` call would run, validated once for both the preview and the call.
fn command_arg(args: &Value) -> Result<String> {
    let command = string_arg(args, "command")?;
    if command.trim().is_empty() {
        return Err(Error::Message("`command` must not be empty".into()));
    }
    Ok(command)
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
