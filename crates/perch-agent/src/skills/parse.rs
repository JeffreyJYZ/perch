//! One `SKILL.md`, read: the front matter it opens with, and the body it carries after it.
//!
//! The front matter is a leading `---` line, `key: value` lines, and a closing `---`; everything
//! after that fence is the body. A file that does not open with the block is passed over — a
//! stray markdown file must not be loaded under a name it never stated. A missing `name` falls
//! back to the directory the file sits in, and a missing `description` is simply absent.
//!
//! A body larger than its share is cut on a character boundary with the cut named in the text, so
//! the model can tell which half of the instructions it is missing.

use std::path::Path;

use super::{MAX_BODY_BYTES, Skill};

/// One file, read: `None` when it has no front matter.
pub(super) fn read(path: &Path) -> Option<Skill> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let (front, body) = front_matter(&text)?;
    let (name, description) = fields(front);
    Some(Skill {
        name: name.unwrap_or_else(|| directory(path)),
        description,
        body: cut(body, path),
        path: path.to_path_buf(),
    })
}

/// The directory a file sits in, which names a skill that forgot to name itself.
fn directory(path: &Path) -> String {
    path.parent().and_then(Path::file_name).map_or_else(
        || "skill".to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// The front matter and the body: `None` when the file does not open with the block.
fn front_matter(text: &str) -> Option<(&str, &str)> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let first = text.split_inclusive('\n').next()?;
    if first.trim_end() != "---" {
        return None;
    }
    let mut offset = first.len();
    let mut opening = None;
    for line in text[offset..].split_inclusive('\n') {
        if line.trim_end() == "---" {
            opening = Some(offset);
            offset += line.len();
            break;
        }
        offset += line.len();
    }
    let start = opening?;
    Some((&text[first.len()..start], &text[offset..]))
}

/// The `name` and `description` a front matter block states, in whichever order they appear.
fn fields(front: &str) -> (Option<String>, Option<String>) {
    let mut name = None;
    let mut description = None;
    for line in front.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = unquote(value);
        if value.is_empty() {
            continue;
        }
        match key.trim().to_lowercase().as_str() {
            "name" => name = Some(value),
            "description" => description = Some(value),
            _ => {}
        }
    }
    (name, description)
}

/// A value with the quotes a person may have wrapped it in taken off.
fn unquote(value: &str) -> String {
    let trimmed = value.trim();
    for quote in ['"', '\''] {
        if let Some(inner) = trimmed
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner.to_string();
        }
    }
    trimmed.to_string()
}

/// A body cut to its share, with the cut named so the model knows what it is missing.
fn cut(body: &str, path: &Path) -> String {
    let body = body.trim();
    if body.len() <= MAX_BODY_BYTES {
        return body.to_string();
    }
    let end = (0..=MAX_BODY_BYTES)
        .rev()
        .find(|index| body.is_char_boundary(*index))
        .unwrap_or(0);
    format!(
        "{}\n\n({} is {} bytes of body; only the first {end} were read because a skill may \
         contribute {MAX_BODY_BYTES} bytes. The rest was cut.)",
        &body[..end],
        path.display(),
        body.len(),
    )
}
