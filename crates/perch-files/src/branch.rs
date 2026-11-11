//! The worktree's branch line: its name, upstream, and how far it has drifted.
//!
//! `git status --porcelain=v2 --branch` prints the four headers this needs in the same
//! walk that reports status, and it is the one form git documents as stable. A plain
//! directory (or a machine with no `git`) yields nothing rather than an error, so the
//! file browser still opens where `git` cannot answer.

use std::path::Path;
use std::process::Command;

use crate::process;
use crate::status::GIT_TIMEOUT;

/// One worktree's branch, read from `git status --porcelain=v2 --branch`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    pub name: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub detached: bool,
}

/// Reads the branch of `root`; a directory that is not a repository yields `None`.
pub fn branch_of(root: &Path) -> Option<Branch> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(root).arg("--no-optional-locks").args([
        "status",
        "--porcelain=v2",
        "--branch",
    ]);
    match process::run(cmd, GIT_TIMEOUT) {
        Ok(captured) if captured.ok => parse_branch(&String::from_utf8_lossy(&captured.stdout)),
        _ => None,
    }
}

/// Parses the `# branch.*` headers; `None` when git printed no head line at all.
///
/// A detached head has no name, so it is reported as `HEAD` with the flag set rather than
/// as an empty string. Without an upstream there is nothing to be ahead of or behind of,
/// so both counts stay at zero.
fn parse_branch(text: &str) -> Option<Branch> {
    let mut head = None;
    let mut upstream = None;
    let mut ahead = 0;
    let mut behind = 0;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("# branch.head ") {
            head = Some(value.trim_end().to_string());
        } else if let Some(value) = line.strip_prefix("# branch.upstream ") {
            upstream = Some(value.trim_end().to_string());
        } else if let Some(value) = line.strip_prefix("# branch.ab ") {
            let mut fields = value.split_whitespace();
            ahead = count(fields.next(), '+');
            behind = count(fields.next(), '-');
        }
    }
    let head = head?;
    let detached = head == "(detached)";
    Some(Branch {
        name: if detached { "HEAD".to_string() } else { head },
        upstream,
        ahead,
        behind,
        detached,
    })
}

/// The number after `sign` in a `+3` / `-1` field, or zero when it does not parse.
fn count(field: Option<&str>, sign: char) -> u32 {
    field
        .and_then(|field| field.strip_prefix(sign))
        .and_then(|digits| digits.parse().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
