//! What a saved rule narrows on, and whether one call falls inside it.
//!
//! A rule narrows on the argument that distinguishes one call from another: the command a shell
//! call runs, or the path a file call touches. Nothing here reads a tool name, so a new tool with
//! a `path` or `command` argument is covered without this module learning about it.

use std::path::Path;

use perch_tools::confine;
use serde_json::Value;

/// The argument a rule narrows on: a command if the call runs one, else a path.
pub(super) fn pattern_of(args: &Value) -> Option<String> {
    command(args).or_else(|| path(args))
}

pub(super) fn command(args: &Value) -> Option<String> {
    args.get("command")
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub(super) fn path(args: &Value) -> Option<String> {
    args.get("path").and_then(Value::as_str).map(str::to_string)
}

/// Whether `candidate` is `saved` or inside it, with both confined to the worktree.
///
/// `false` on any resolution failure: an argument that cannot be confined is exactly the case a
/// saved rule must not open. Confining before comparing is what stops `..`, an absolute path and
/// a symlink from turning a rule for `src/` into a rule for the rest of the disk.
pub(super) fn under(root: &Path, candidate: &str, saved: &str) -> bool {
    let Ok(target) = confine(root, Path::new(candidate)) else {
        return false;
    };
    let Ok(base) = confine(root, Path::new(saved)) else {
        return false;
    };
    target.starts_with(&base)
}
