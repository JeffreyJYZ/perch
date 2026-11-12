//! What the tool enforces on its own: the label, the brief, and the answer.

use perch_tools::{Error, Result};
use serde_json::Value;

use super::limits::{LABEL_MAX_CHARS, PROMPT_MAX_BYTES};

/// The line that names the task above its answer.
pub(super) fn heading(description: &str, rounds: Option<usize>) -> String {
    match rounds {
        Some(1) => format!("task \"{description}\" -> done in 1 round"),
        Some(rounds) => format!("task \"{description}\" -> done in {rounds} rounds"),
        None => format!("task \"{description}\" -> done"),
    }
}

/// The label: one line, so a heading stays a heading.
pub(super) fn label(args: &Value) -> Result<String> {
    let raw = args
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let words: Vec<&str> = raw.split_whitespace().collect();
    if words.is_empty() {
        return Err(Error::Message(
            "the task tool needs a `description`: a few words naming the task".to_string(),
        ));
    }
    let one_line = words.join(" ");
    if one_line.chars().count() <= LABEL_MAX_CHARS {
        return Ok(one_line);
    }
    let cut: String = one_line.chars().take(LABEL_MAX_CHARS).collect();
    Ok(format!("{cut}…"))
}

/// The brief, refused over the cap rather than cut: half a brief is a child set up to fail.
pub(super) fn brief(args: &Value) -> Result<String> {
    let prompt = args
        .get("prompt")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if prompt.trim().is_empty() {
        return Err(Error::Message(
            "the task tool needs a `prompt`: the child starts with no other context".to_string(),
        ));
    }
    if prompt.len() > PROMPT_MAX_BYTES {
        return Err(Error::Message(format!(
            "the task prompt is {} bytes, over the {PROMPT_MAX_BYTES}-byte cap for one brief; \
             split the work or leave out what the child does not need",
            prompt.len()
        )));
    }
    Ok(prompt.to_string())
}

/// Keeps an answer up to its cap on a character boundary, naming the cut when it makes one.
pub(super) fn cut(text: String, cap: usize) -> String {
    if text.len() <= cap {
        return text;
    }
    let total = text.len();
    let mut end = cap;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n[truncated: the child answered {total} bytes, and a result carries {cap}]",
        &text[..end]
    )
}
