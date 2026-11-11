//! What a title may be: one line, at most six words, no quotes, no prefix, no trailing stop.
//!
//! A provider is not a formatter, so the reply is cleaned here rather than trusted: the first
//! line is kept, a `Title:` prefix and quotes are stripped, trailing punctuation goes, and the
//! rest is cut at the word count. A reply that names nothing leaves the folder name alone.

use perch_store::SessionMeta;

/// How many words an auto-title may have.
pub const WORDS: usize = 6;

/// The folder name a session is created with.
pub(crate) fn default_title(meta: &SessionMeta) -> String {
    meta.worktree.file_name().map_or_else(
        || "session".to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// One line, at most six words, no quotes, no `Title:` and no trailing punctuation.
#[must_use]
pub fn tidy(raw: &str) -> Option<String> {
    let line = raw.lines().next()?.trim();
    let line = line
        .strip_prefix("Title:")
        .or_else(|| line.strip_prefix("title:"))
        .unwrap_or(line)
        .trim();
    let line = line.trim_matches(['"', '\'', '“', '”', '‘', '’']).trim();
    let line = line.trim_end_matches([
        '.', ',', ';', ':', '!', '?', '。', '，', '；', '：', '！', '？',
    ]);
    let words: Vec<&str> = line.split_whitespace().take(WORDS).collect();
    let title = words.join(" ").trim().to_string();
    if title.is_empty() { None } else { Some(title) }
}

#[cfg(test)]
mod tests {
    use super::tidy;

    #[test]
    fn a_title_is_one_line_and_at_most_six_words() {
        assert_eq!(
            tidy("Fix the terminal grid"),
            Some("Fix the terminal grid".to_string())
        );
        assert_eq!(
            tidy("one two three four five six seven eight"),
            Some("one two three four five six".to_string())
        );
        assert_eq!(
            tidy("Title: \"Fix the grid.\"\nsecond line"),
            Some("Fix the grid".to_string())
        );
        assert_eq!(tidy("  \n "), None);
        assert_eq!(tidy("\n"), None);
        assert!(tidy("a\nb").is_some_and(|title| !title.contains('\n')));
    }
}
