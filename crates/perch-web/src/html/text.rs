//! The words: entity decoding, soft-wrapped source lines, and spacing normalisation.

use super::entities;

/// Reads text, decoding entities. A source line break is a soft wrap in HTML, so it becomes a
/// space; only a block element or `<br>` starts a new line.
pub fn words(out: &mut String, text: &str) {
    for (index, chunk) in entities::decode(text).split(['\n', '\r']).enumerate() {
        if index > 0 {
            out.push(' ');
        }
        out.push_str(chunk);
    }
}

/// Collapses spacing and runs of blank lines: one space between words, one blank line between
/// paragraphs, no leading or trailing blank line.
pub fn tidy(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut blank = false;
    for line in raw.lines() {
        let line = squeeze(line);
        if line.is_empty() {
            blank = !out.is_empty();
            continue;
        }
        if blank {
            out.push('\n');
        }
        blank = false;
        out.push_str(&line);
        out.push('\n');
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

fn squeeze(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut space = false;
    for word in line.split_whitespace() {
        if space {
            out.push(' ');
        }
        space = true;
        out.push_str(word);
    }
    out
}
