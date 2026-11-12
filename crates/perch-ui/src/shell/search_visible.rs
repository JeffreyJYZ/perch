//! What a transcript item's words actually are once markdown's markers are gone.
//!
//! A search runs over what the column draws — a user's turn, a reply, a thought, a note, a tool
//! card's header and its whole output — never over the markdown source. `## Heading` is drawn as
//! `Heading`, so the words are read back through the same small pass that strips the markers, and
//! the needle is put through it too: a heading searched for with its hashes finds the heading, and
//! so does the plain word.

use crate::transcript::{Item, summary_line};

/// What a reader sees of `item`, with markdown's markers taken out.
#[must_use]
pub fn visible_text(item: &Item) -> String {
    match item {
        Item::User { text } | Item::Note(text) | Item::Reasoning { text } => text.clone(),
        Item::Assistant { text } => markdown_visible(text),
        Item::Tool {
            name,
            target,
            output,
            ..
        } => {
            let mut words = name.clone();
            if let Some(target) = target {
                words.push(' ');
                words.push_str(target);
            }
            words.push('\n');
            words.push_str(output);
            words
        }
        Item::Summary {
            tools,
            failed,
            tokens,
        } => summary_line(*tools, *failed, *tokens),
    }
}

/// The needle as it is looked for: markdown's markers taken out, lowercased.
///
/// This is what makes a search for `## Heading` and a search for `Heading` the same search.
#[must_use]
pub fn visible_needle(needle: &str) -> String {
    markdown_visible(needle).trim().to_lowercase()
}

/// `source` as it is drawn: fences, heading hashes, quote and list markers and emphasis dropped.
///
/// The pass is deliberately small. It answers what a search sees, not what a renderer builds, and
/// a marker it does not know is a character a reader would also see.
pub(super) fn markdown_visible(source: &str) -> String {
    let mut out = String::new();
    let mut fenced = false;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        out.push_str(&inline_visible(without_prefixes(trimmed)));
        out.push('\n');
    }
    out.trim_end().to_string()
}

/// `line` without the block markers in front of it: a heading's hashes, a quote's `>` and a list's
/// bullet or number.
fn without_prefixes(line: &str) -> &str {
    let mut rest = line;
    let hashes = rest.chars().take_while(|c| *c == '#').count();
    if hashes > 0 && rest[hashes..].starts_with(' ') {
        rest = rest[hashes..].trim_start();
    }
    loop {
        let stripped = rest
            .strip_prefix("> ")
            .or_else(|| rest.strip_prefix("- "))
            .or_else(|| rest.strip_prefix("* "))
            .or_else(|| rest.strip_prefix("+ "))
            .or_else(|| numbered_item(rest));
        match stripped {
            Some(after) => rest = after,
            None => return rest,
        }
    }
}

/// What follows an ordered list marker (`1. `) when the line starts with one.
fn numbered_item(line: &str) -> Option<&str> {
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    let rest = line[digits..].strip_prefix(". ")?;
    (digits > 0).then_some(rest)
}

/// `line` with emphasis and code markers gone and a link's text kept in place of its target.
fn inline_visible(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut at = 0;
    while at < chars.len() {
        match chars[at] {
            '*' | '_' | '`' | '~' => at += 1,
            '!' if chars.get(at + 1) == Some(&'[') => at += 1,
            '[' => match chars[at + 1..].iter().position(|c| *c == ']') {
                Some(offset) => {
                    let close = at + 1 + offset;
                    out.extend(chars[at + 1..close].iter().copied());
                    at = close + 1;
                    if chars.get(at) == Some(&'(') {
                        at = chars[at + 1..]
                            .iter()
                            .position(|c| *c == ')')
                            .map_or(chars.len(), |offset| at + 1 + offset + 1);
                    }
                }
                None => {
                    out.push(chars[at]);
                    at += 1;
                }
            },
            c => {
                out.push(c);
                at += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{visible_needle, visible_text};
    use crate::transcript::Item;

    fn assistant(text: &str) -> Item {
        Item::Assistant {
            text: text.to_string(),
        }
    }

    #[test]
    fn a_heading_is_drawn_without_its_hashes() {
        let shown = visible_text(&assistant("## Build and test\n\nrun it twice"));
        assert!(
            shown.starts_with("Build and test") && !shown.contains("##"),
            "the hashes are markers, not words: {shown:?}"
        );
    }

    #[test]
    fn emphasis_and_link_targets_are_not_text_to_find() {
        assert_eq!(visible_needle("**loud**"), "loud");
        assert_eq!(
            visible_text(&assistant("a [link](https://example.com) and `code`")),
            "a link and code",
            "the link's words stay and its target does not"
        );
    }
}
