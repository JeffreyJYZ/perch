//! Reading a unified diff into file patches, one hunk reader per file section.

use super::patch_hunk::{Hunk, parse_hunk};
use crate::{Error, Result};

/// One file's section of a diff: its two header paths and its hunks, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FilePatch {
    /// `None` when the header said `/dev/null`, so the file is being created.
    pub(super) old_path: Option<String>,
    /// `None` when the header said `/dev/null`, so the file is being deleted.
    pub(super) new_path: Option<String>,
    pub(super) hunks: Vec<Hunk>,
}

/// A hunk line, with the terminator the patch states (a `\ No newline` marker removes it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Line {
    Context(String),
    Remove(String),
    Add(String),
}

impl FilePatch {
    /// The path a person would name: the target, or the source when the file is deleted.
    pub(super) fn path(&self) -> Result<&str> {
        self.new_path
            .as_deref()
            .or(self.old_path.as_deref())
            .ok_or_else(|| {
                Error::Message("a file header names neither a source nor a target path".to_string())
            })
    }

    pub(super) fn added(&self) -> usize {
        self.count(|line| matches!(line, Line::Add(_)))
    }

    pub(super) fn removed(&self) -> usize {
        self.count(|line| matches!(line, Line::Remove(_)))
    }

    fn count(&self, wanted: impl Fn(&Line) -> bool) -> usize {
        self.hunks
            .iter()
            .flat_map(|hunk| &hunk.lines)
            .filter(|line| wanted(line))
            .count()
    }
}

/// Parses a whole diff, refusing anything it cannot read exactly.
pub(super) fn parse(text: &str) -> Result<Vec<FilePatch>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut patches = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        if !line.starts_with("--- ") {
            if line.trim().is_empty() {
                index += 1;
                continue;
            }
            return Err(Error::Message(format!(
                "diff line {} is not a `--- ` file header: {line}",
                index + 1
            )));
        }
        let old_path = header_path(line)?;
        let target = lines
            .get(index + 1)
            .copied()
            .filter(|l| l.starts_with("+++ "));
        let target = target.ok_or_else(|| {
            Error::Message(format!(
                "the `--- ` header at diff line {} is not followed by `+++ `",
                index + 1
            ))
        })?;
        let new_path = header_path(target)?;
        index += 2;

        let mut hunks = Vec::new();
        while index < lines.len() && lines[index].starts_with("@@") {
            let (hunk, next) = parse_hunk(&lines, index)?;
            hunks.push(hunk);
            index = next;
        }
        if hunks.is_empty() {
            let path = new_path.as_deref().or(old_path.as_deref()).unwrap_or("?");
            return Err(Error::Message(format!(
                "{path}: the file header has no hunks"
            )));
        }
        patches.push(FilePatch {
            old_path,
            new_path,
            hunks,
        });
    }
    Ok(patches)
}

/// The path from one `--- ` or `+++ ` line, with its `a/` or `b/` prefix and timestamp removed.
fn header_path(line: &str) -> Result<Option<String>> {
    let rest = line
        .strip_prefix("--- ")
        .or_else(|| line.strip_prefix("+++ "))
        .unwrap_or(line);
    let path = rest.split('\t').next().unwrap_or(rest).trim();
    if path == "/dev/null" {
        return Ok(None);
    }
    let path = path
        .strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path);
    if path.is_empty() {
        return Err(Error::Message(format!("empty path in `{line}`")));
    }
    Ok(Some(path.to_string()))
}
