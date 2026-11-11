//! A small HTML-to-text reduction: no parser crate, and every rule is a test.
//!
//! The reduction is honest about what it is: tags are dropped, a few block elements become line
//! breaks, headings become `#` when asked, links become `text (url)`, and script/style/noscript/svg
//! and comments never reach the model. It does not try to reflow tables or preserve inline code.
//!
//! The work is split by responsibility: [`drop`] removes what is never text, [`tags`] turns one tag
//! into breaks and markdown, [`text`] decodes and normalises the words, and this file assembles
//! them.

mod drop;
mod entities;
mod tags;
#[cfg(test)]
mod tests;
mod text;

use std::fmt::Write as _;

pub use tags::attribute;
use tags::{open_href, tag};
use text::{tidy, words};

/// What the reduction keeps beyond the words themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    pub headings: bool,
    pub links: bool,
}

/// `#` for headings and `text (url)` for links: what `webfetch` hands the model by default.
pub const MARKDOWN: Style = Style {
    headings: true,
    links: true,
};

/// Words only, one link label per anchor, no markdown syntax.
pub const PLAIN: Style = Style {
    headings: false,
    links: false,
};

/// Reduces one HTML document to text.
pub fn reduce(html: &str, style: Style) -> String {
    let cleaned = drop::blocks(html);
    let mut out = String::with_capacity(cleaned.len() / 2);
    let mut rest = cleaned.as_str();
    while let Some(index) = rest.find('<') {
        words(&mut out, &rest[..index]);
        let tail = &rest[index..];
        let Some(close) = tail.find('>') else {
            words(&mut out, &tail[1..]);
            rest = "";
            break;
        };
        let inner = &tail[1..close];
        let after = &tail[close + 1..];
        if style.links
            && let Some(href) = open_href(inner)
            && let Some(next) = anchor(after, &mut out, &href)
        {
            rest = next;
            continue;
        }
        tag(&mut out, inner, style);
        rest = after;
    }
    words(&mut out, rest);
    tidy(&out)
}

/// Consumes an anchor whose opening tag and `href` were already matched, emitting its label and,
/// when both are present, the URL after it. Answers what follows the closing tag.
fn anchor<'a>(rest: &'a str, out: &mut String, href: &str) -> Option<&'a str> {
    let end = rest.find("</a>")?;
    let label = reduce(&rest[..end], PLAIN);
    let label = label.trim();
    if label.is_empty() {
        out.push_str(href);
    } else if href.is_empty() || href == label {
        out.push_str(label);
    } else {
        let _ = write!(out, "{label} ({href})");
    }
    Some(&rest[end + 4..])
}
