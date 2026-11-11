//! One tag at a time: element names, block breaks, heading levels and attribute values.

use super::Style;
use super::entities;

/// Elements that end a line.
const BLOCK: [&str; 24] = [
    "p",
    "div",
    "table",
    "tr",
    "ul",
    "ol",
    "pre",
    "blockquote",
    "section",
    "article",
    "header",
    "footer",
    "nav",
    "main",
    "dl",
    "dt",
    "dd",
    "figure",
    "figcaption",
    "form",
    "fieldset",
    "thead",
    "tbody",
    "address",
];

/// Applies one tag that is not part of a link.
pub fn tag(out: &mut String, inner: &str, style: Style) {
    let closing = inner.starts_with('/');
    let element = name(inner);
    let element = element.as_str();
    if let Some(level) = heading(element) {
        out.push_str("\n\n");
        if !closing && style.headings {
            for _ in 0..level {
                out.push('#');
            }
            out.push(' ');
        }
        return;
    }
    match element {
        "br" => out.push('\n'),
        "li" => out.push_str(if closing { "\n" } else { "- " }),
        "hr" => out.push_str("\n\n"),
        "td" | "th" => out.push(if closing { '\n' } else { ' ' }),
        _ if BLOCK.contains(&element) => out.push('\n'),
        _ => {}
    }
}

fn heading(element: &str) -> Option<usize> {
    let level: usize = element.strip_prefix('h')?.parse().ok()?;
    (1..=6).contains(&level).then_some(level)
}

/// The tag's element name, lowercased, without the `/` of a closing tag or any attributes.
pub fn name(inner: &str) -> String {
    inner
        .trim_start_matches(['/', '!', '?'])
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == ':')
        .collect::<String>()
        .to_ascii_lowercase()
}

/// The `href` of an opening `<a>` tag, decoded, or `None` when this tag is not such an anchor.
pub fn open_href(inner: &str) -> Option<String> {
    if name(inner) != "a" {
        return None;
    }
    attribute(inner, "href")
}

/// One attribute value, unquoted or either quote style, with entities decoded.
///
/// The name is matched on a boundary, so `data-href` is not `href`, and the value is read from the
/// original tag rather than its lowercased copy, because a URL's path is case-sensitive.
pub fn attribute(inner: &str, key: &str) -> Option<String> {
    let lower = inner.to_ascii_lowercase();
    let mut at = 0;
    while let Some(index) = lower[at..].find(key) {
        let start = at + index;
        let after = start + key.len();
        let before = inner[..start].chars().next_back();
        let boundary = before.is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-');
        if boundary && let Some(value) = inner[after..].trim_start().strip_prefix('=') {
            return Some(entities::decode(raw_value(value.trim_start())));
        }
        at = after;
    }
    None
}

fn raw_value(value: &str) -> &str {
    match value.chars().next() {
        Some(quote @ ('"' | '\'')) => {
            let body = &value[1..];
            &body[..body.find(quote).unwrap_or(body.len())]
        }
        Some(_) => value.split([' ', '>', '\t']).next().unwrap_or(""),
        None => "",
    }
}
