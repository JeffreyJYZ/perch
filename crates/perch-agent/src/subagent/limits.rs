//! The three sizes the tool enforces on its own.

/// The most a task's brief may be: a brief is a brief, and a child starts with no history.
pub const PROMPT_MAX_BYTES: usize = 64 * 1024;

/// The most one child's answer may contribute to a result.
pub const RESULT_MAX_BYTES: usize = 64 * 1024;

/// The most of a label that reaches the result line.
pub const LABEL_MAX_CHARS: usize = 100;
