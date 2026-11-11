//! What changed: one file's diff, or the whole worktree's, capped and never binary bytes.
//!
//! `git diff` is the contract for a tracked change, read the same way `status.rs` reads
//! status: a 30 s timeout, both streams captured, and `--no-optional-locks` because this
//! crate never mutates a worktree. An untracked file has no diff to read, so it is rendered
//! here as an all-added diff — a person asking "what changed" about a new file wants to see
//! the new file. A diff view scrolls, so every answer stops at 256 KiB with the cut named.

use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::process;
use crate::status::GIT_TIMEOUT;
use crate::tree::confine_in;

/// A diff never returns more than this many bytes; the cut is named in the text.
pub const DIFF_MAX_BYTES: usize = 256 * 1024;

/// The uncommitted change in one file.
///
/// `None` when nothing changed, when `path` leaves `root`, or when `root` is not a
/// repository. A tracked file is git's own diff; an untracked file is read from disk and
/// rendered as added lines.
pub fn file_diff(root: &Path, path: &Path) -> Option<String> {
    let (root, target) = confine_in(root, path).ok()?;
    let relative = target.strip_prefix(&root).ok()?.to_path_buf();
    if tracked(&root, &relative) {
        let text = git_text(&root, &diff_args(&relative))?;
        return (!text.is_empty()).then(|| cap(text));
    }
    added_diff(&relative, &target).map(cap)
}

/// Every uncommitted change at once, untracked files included, for a view that scrolls.
///
/// `None` only when `root` is not a repository; a clean worktree is an empty string.
pub fn worktree_diff(root: &Path) -> Option<String> {
    let root = std::fs::canonicalize(root).ok()?;
    let mut text = git_text(&root, &[OsStr::new("diff"), OsStr::new("--no-color")])?;
    let untracked = git_text(
        &root,
        &[
            OsStr::new("ls-files"),
            OsStr::new("--others"),
            OsStr::new("--exclude-standard"),
            OsStr::new("-z"),
            OsStr::new("--"),
        ],
    )?;
    for entry in untracked.split('\0').filter(|entry| !entry.is_empty()) {
        if text.len() >= DIFF_MAX_BYTES {
            break;
        }
        let relative = PathBuf::from(entry);
        if let Some(block) = added_diff(&relative, &root.join(&relative)) {
            text.push_str(&block);
        }
    }
    Some(cap(text))
}

fn diff_args(relative: &Path) -> [&OsStr; 4] {
    [
        OsStr::new("diff"),
        OsStr::new("--no-color"),
        OsStr::new("--"),
        relative.as_os_str(),
    ]
}

/// One git subcommand's stdout as text; `None` when git refused or is absent.
fn git_text(root: &Path, args: &[&OsStr]) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(root)
        .arg("--no-optional-locks")
        .args(args);
    let captured = process::run(cmd, GIT_TIMEOUT).ok().filter(|c| c.ok)?;
    Some(String::from_utf8_lossy(&captured.stdout).into_owned())
}

fn tracked(root: &Path, relative: &Path) -> bool {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(root)
        .arg("--no-optional-locks")
        .args(["ls-files", "--error-unmatch", "--"])
        .arg(relative);
    matches!(process::run(cmd, GIT_TIMEOUT), Ok(captured) if captured.ok)
}

/// A file with no index entry, rendered as git would show a newly added file.
///
/// A binary file is named rather than copied, because a diff view cannot draw bytes and a
/// wire should not carry them.
fn added_diff(relative: &Path, target: &Path) -> Option<String> {
    let file = std::fs::File::open(target).ok()?;
    let mut bytes = Vec::new();
    file.take(DIFF_MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    let show = relative.to_string_lossy();
    let mut out = format!("diff --git a/{show} b/{show}\n");
    if bytes.contains(&0) {
        out.push_str(&format!("Binary file {show} is not shown\n"));
        return Some(out);
    }
    let text = String::from_utf8_lossy(&bytes);
    out.push_str(&format!(
        "new file mode 100644\n--- /dev/null\n+++ b/{show}\n"
    ));
    let lines = text.lines().count();
    if lines > 0 {
        out.push_str(&format!("@@ -0,0 +1,{lines} @@\n"));
    }
    for line in text.lines() {
        out.push('+');
        out.push_str(line);
        out.push('\n');
    }
    if !text.is_empty() && !text.ends_with('\n') {
        out.push_str("\\ No newline at end of file\n");
    }
    Some(out)
}

/// Cuts `text` at the cap on a character boundary and says how much of it was shown.
fn cap(text: String) -> String {
    if text.len() <= DIFF_MAX_BYTES {
        return text;
    }
    let total = text.len();
    let mut end = DIFF_MAX_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut cut = text[..end].to_string();
    cut.push_str(&format!("\n[diff cut: {end} of {total} bytes shown]\n"));
    cut
}

#[cfg(test)]
mod tests;
