//! perch-tools: the built-in tools, the registry that owns them, and path confinement.
//!
//! Tools are registered, never enumerated. `ToolRegistry::call` dispatches by name only, so a
//! plugin tool added later takes exactly the path a built-in takes, and `specs()` is computed
//! from whatever is registered rather than from a static list.

mod builtin;
mod confine;
mod registry;

use std::path::PathBuf;
use std::time::Duration;

pub use builtin::{
    BASH_MAX_OUTPUT, BASH_TIMEOUT, Bash, GREP_MAX_OUTPUT, Glob, Grep, JOB_MAX_CONCURRENT, Jobs,
    LIST_MAX_ENTRIES, Patch, READ_MAX_BYTES, builtin_tools,
};
pub use confine::confine;
pub use registry::ToolRegistry;

/// Everything this crate can fail at.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{path} is outside the worktree root")]
    OutsideRoot { path: PathBuf },
    #[error("no tool named {0}")]
    UnknownTool(String),
    #[error("{program} did not finish within {after:?} and was killed")]
    Timeout { program: String, after: Duration },
    #[error("{0}")]
    Message(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// What the model is told about one tool: the OpenAI-compatible function shape.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema for the arguments object, which is also what a plugin declares.
    pub parameters: serde_json::Value,
}

/// The one directory a call may touch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCtx {
    pub root: PathBuf,
}

/// What the approval view and the transcript need back from a call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Preview {
    /// A unified diff of an edit.
    Diff(String),
    /// The command a shell call would run.
    Command(String),
}

/// A tool's result: text for the model, and an optional preview for the approval view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutput {
    pub text: String,
    pub preview: Option<Preview>,
}

impl ToolOutput {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            preview: None,
        }
    }

    pub fn with_preview(text: impl Into<String>, preview: Preview) -> Self {
        Self {
            text: text.into(),
            preview: Some(preview),
        }
    }
}

/// One callable tool. A plugin implements exactly this, so nothing here is built-in-only.
pub trait Tool: Send + Sync {
    fn spec(&self) -> ToolSpec;

    /// `write`, `edit` and `bash` ask first; reads do not.
    fn needs_approval(&self) -> bool;

    fn call(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<ToolOutput>;

    /// What [`Tool::call`] would change, computed without running it.
    ///
    /// The approval view is shown this, not the arguments, so a person approves the change rather
    /// than a description of it. The default is "nothing to show": a read-only tool, a plugin tool
    /// that does not implement it, and a test tool all owe the view no preview. Anything that
    /// mutates should implement it, and an error here denies the call — a change that cannot be
    /// shown is not approved blind.
    fn preview(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<Option<Preview>> {
        let _ = (args, ctx);
        Ok(None)
    }
}
