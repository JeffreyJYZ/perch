//! What is never readable text: comments, and the contents of script, style, noscript and svg.

use super::tags::name;

/// Elements whose content is never readable text.
const DROPPED: [&str; 4] = ["script", "style", "noscript", "svg"];

/// Drops comments and the contents of [`DROPPED`] elements, leaving every other tag in place.
pub fn blocks(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    loop {
        let Some(index) = rest.find('<') else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..index]);
        let tail = &rest[index..];
        if let Some(after) = comment(tail) {
            rest = after;
            continue;
        }
        let element = name(&tail[1..]);
        if DROPPED.contains(&element.as_str()) && !tail[1..].starts_with('/') {
            rest = element_content_end(tail, &element);
            continue;
        }
        out.push('<');
        rest = &tail[1..];
    }
}

fn comment(tail: &str) -> Option<&str> {
    let body = tail.strip_prefix("<!--")?;
    Some(match body.find("-->") {
        Some(end) => &body[end + 3..],
        None => "",
    })
}

/// Everything after a dropped element's closing tag, or nothing when it never closes.
fn element_content_end<'a>(tail: &'a str, element: &str) -> &'a str {
    let lower = tail.to_ascii_lowercase();
    let Some(head) = lower.find('>') else {
        return "";
    };
    let close = format!("</{element}");
    let Some(index) = lower[head..].find(&close) else {
        return "";
    };
    match lower[head + index..].find('>') {
        Some(end) => &tail[head + index + end + 1..],
        None => "",
    }
}
