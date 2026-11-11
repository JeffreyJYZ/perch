//! Reading one `@@` hunk: its ranges, its exact lines and their terminators.

use super::patch_diff::Line;
use crate::{Error, Result};

/// One `@@` block: where it starts in the old file and the exact lines it carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Hunk {
    pub(super) old_start: usize,
    pub(super) old_len: usize,
    pub(super) new_len: usize,
    pub(super) lines: Vec<Line>,
}

/// Reads one hunk body, bounded by the counts in its header so a line such as `--- x` inside a
/// hunk is a removal, not a new file.
pub(super) fn parse_hunk(lines: &[&str], start: usize) -> Result<(Hunk, usize)> {
    let header = lines[start];
    let (old_start, old_len, new_len) = header_counts(header)?;
    let mut index = start + 1;
    let mut body: Vec<Line> = Vec::new();
    let mut seen_old = 0;
    let mut seen_new = 0;

    while index < lines.len() && (seen_old < old_len || seen_new < new_len) {
        let line = lines[index];
        // An empty context line is often stripped of its single space by an editor or a paste;
        // read it as context rather than refusing a diff that is otherwise exact.
        let (tag, content) = match line.chars().next() {
            None => (' ', "\n".to_string()),
            Some(tag) => (tag, format!("{}\n", &line[tag.len_utf8()..])),
        };
        if tag == '\\' {
            strip_terminator(body.last_mut());
            index += 1;
            continue;
        }
        match tag {
            ' ' => {
                seen_old += 1;
                seen_new += 1;
                body.push(Line::Context(content));
            }
            '-' => {
                seen_old += 1;
                body.push(Line::Remove(content));
            }
            '+' => {
                seen_new += 1;
                body.push(Line::Add(content));
            }
            _ => return Err(bad_hunk(header, &format!("unexpected line `{line}`"))),
        }
        index += 1;
    }
    while index < lines.len() && lines[index].starts_with('\\') {
        strip_terminator(body.last_mut());
        index += 1;
    }
    if seen_old != old_len || seen_new != new_len {
        return Err(bad_hunk(
            header,
            &format!("the body has {seen_old}/{old_len} old and {seen_new}/{new_len} new lines"),
        ));
    }
    Ok((
        Hunk {
            old_start,
            old_len,
            new_len,
            lines: body,
        },
        index,
    ))
}

/// `@@ -1,3 +1,4 @@` answers `(1, 3, 4)`; an omitted count means one line.
fn header_counts(header: &str) -> Result<(usize, usize, usize)> {
    let body = header
        .strip_prefix("@@ -")
        .ok_or_else(|| bad_hunk(header, "not a `@@ -l,c +l,c @@` header"))?;
    let (old, rest) = body
        .split_once(" +")
        .ok_or_else(|| bad_hunk(header, "no `+` range"))?;
    let (new, _) = rest
        .split_once(" @@")
        .ok_or_else(|| bad_hunk(header, "no closing `@@`"))?;
    let (old_start, old_len) = range(old).map_err(|_| bad_hunk(header, "bad old range"))?;
    let (_, new_len) = range(new).map_err(|_| bad_hunk(header, "bad new range"))?;
    Ok((old_start, old_len, new_len))
}

fn range(text: &str) -> Result<(usize, usize)> {
    let (start, len) = text.split_once(',').unwrap_or((text, "1"));
    let start = start
        .trim()
        .parse::<usize>()
        .map_err(|_| Error::Message(format!("bad line number `{text}`")))?;
    let len = len
        .trim()
        .parse::<usize>()
        .map_err(|_| Error::Message(format!("bad line count `{text}`")))?;
    Ok((start, len))
}

/// `\ No newline at end of file` applies to the line before it.
fn strip_terminator(line: Option<&mut Line>) {
    if let Some(line) = line {
        let text = match line {
            Line::Context(text) | Line::Remove(text) | Line::Add(text) => text,
        };
        if text.ends_with('\n') {
            text.pop();
        }
    }
}

fn bad_hunk(header: &str, reason: &str) -> Error {
    Error::Message(format!("unreadable hunk `{header}`: {reason}"))
}
