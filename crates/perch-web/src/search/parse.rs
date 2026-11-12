//! Reading DuckDuckGo's HTML result page: `result__a` anchors, `result__snippet` labels.

use crate::html::{self, PLAIN, attribute};

/// One result, after the page's markup has been reduced to words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

const ANCHOR: &str = r#"class="result__a""#;
const SNIPPET: &str = r#"class="result__snippet""#;

/// Every result the page carries, in page order.
pub fn results(page: &str) -> Vec<Hit> {
    let mut hits = Vec::new();
    let mut at = 0;
    while let Some(index) = find_from(page, ANCHOR, at) {
        let Some(head_start) = page[..index].rfind('<') else {
            break;
        };
        let Some(head_len) = page[index..].find('>') else {
            break;
        };
        let head_end = index + head_len;
        let tag = &page[head_start + 1..head_end];
        let Some(href) = attribute(tag, "href") else {
            at = head_end + 1;
            continue;
        };
        let Some(label_len) = page[head_end..].find("</a>") else {
            break;
        };
        let label_end = head_end + label_len;
        let title = one_line(&html::reduce(&page[head_end + 1..label_end], PLAIN));
        let (snippet, next) = snippet_after(page, label_end + 4);
        let url = unwrap(&href);
        if !url.is_empty() && !title.is_empty() {
            hits.push(Hit {
                title,
                url,
                snippet,
            });
        }
        at = next.max(at + 1);
    }
    hits
}

/// Drops results whose URL was already seen, keeping the first, which is the best ranked.
pub fn dedupe(hits: Vec<Hit>) -> Vec<Hit> {
    let mut kept: Vec<Hit> = Vec::with_capacity(hits.len());
    for hit in hits {
        if kept.iter().all(|seen| seen.url != hit.url) {
            kept.push(hit);
        }
    }
    kept
}

/// The snippet belonging to the result that ended just before `from`.
fn snippet_after(page: &str, from: usize) -> (String, usize) {
    let Some(index) = find_from(page, SNIPPET, from) else {
        return (String::new(), page.len());
    };
    let Some(head_len) = page[index..].find('>') else {
        return (String::new(), page.len());
    };
    let head_end = index + head_len;
    let Some(label_len) = page[head_end..].find("</a>") else {
        return (String::new(), page.len());
    };
    let label_end = head_end + label_len;
    (
        one_line(&html::reduce(&page[head_end + 1..label_end], PLAIN)),
        label_end + 4,
    )
}

/// The result's real URL: DuckDuckGo wraps it in a `uddg=` redirect, and some links are
/// protocol-relative.
pub fn unwrap(url: &str) -> String {
    if let Some(index) = url.find("uddg=") {
        let rest = &url[index + 5..];
        let value = rest.split('&').next().unwrap_or(rest);
        let decoded = super::encode::decode(value);
        if !decoded.is_empty() {
            return decoded;
        }
    }
    match url.strip_prefix("//") {
        Some(rest) => format!("https://{rest}"),
        None => url.to_string(),
    }
}

/// One line at most, because a snippet spanning three lines wrecks a result list.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn find_from(page: &str, needle: &str, from: usize) -> Option<usize> {
    page.get(from..)?.find(needle).map(|index| from + index)
}
