//! Applying hunks to one file's text, refusing the first line that does not match.

use super::patch_diff::Line;
use super::patch_hunk::Hunk;
use crate::{Error, Result};

/// Applies every hunk in order. Nothing is written here, so a failure halfway through a diff
/// still leaves the file untouched: the caller only writes what this returns.
pub(super) fn apply(original: &str, hunks: &[Hunk], file: &str) -> Result<String> {
    let lines: Vec<&str> = original.split_inclusive('\n').collect();
    let mut out = String::with_capacity(original.len());
    let mut cursor = 0;

    for (number, hunk) in hunks.iter().enumerate() {
        let at = if hunk.old_len == 0 {
            hunk.old_start
        } else {
            hunk.old_start.saturating_sub(1)
        };
        if at > lines.len() {
            return Err(fail(
                file,
                number,
                hunk,
                &format!("the file has only {} lines", lines.len()),
            ));
        }
        if at < cursor {
            return Err(fail(file, number, hunk, "overlaps the previous hunk"));
        }
        while cursor < at {
            out.push_str(lines[cursor]);
            cursor += 1;
        }
        for line in &hunk.lines {
            let (text, keep) = match line {
                Line::Add(text) => {
                    out.push_str(text);
                    continue;
                }
                Line::Context(text) => (text, true),
                Line::Remove(text) => (text, false),
            };
            let Some(actual) = lines.get(cursor) else {
                return Err(fail(
                    file,
                    number,
                    hunk,
                    "the file ends before the hunk does",
                ));
            };
            if *actual != text.as_str() {
                return Err(fail(
                    file,
                    number,
                    hunk,
                    &format!(
                        "line {} does not match\n  expected {text:?}\n  found    {actual:?}",
                        cursor + 1
                    ),
                ));
            }
            if keep {
                out.push_str(actual);
            }
            cursor += 1;
        }
    }

    while cursor < lines.len() {
        out.push_str(lines[cursor]);
        cursor += 1;
    }
    Ok(out)
}

/// Names the file, the hunk and where in the file it starts, so a caller can fix the diff.
fn fail(file: &str, number: usize, hunk: &Hunk, reason: &str) -> Error {
    Error::Message(format!(
        "{file}: hunk {} (starting at line {}): {reason}",
        number + 1,
        hunk.old_start
    ))
}
