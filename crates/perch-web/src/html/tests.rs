//! The reduction's rules, each pinned by a fixture rather than by a live page.

use super::{MARKDOWN, PLAIN, attribute, reduce};

const PAGE: &str = r#"<!DOCTYPE html>
<html><head>
	<style>body { color: red }</style>
	<script>window.leak = "no";</script>
</head>
<body>
	<!-- a comment that must not survive -->
	<h1>Perch &amp; Rune</h1>
	<p>First   paragraph
	   over two lines.</p>
	<p>Second with a <a href="https://example.com/docs?a=1&amp;b=2">useful link</a> inside.</p>
	<noscript>Enable JS</noscript>
	<svg><path d="M0 0"/></svg>
	<ul><li>one</li><li>two</li></ul>
</body></html>"#;

#[test]
fn dropped_elements_and_comments_never_reach_the_text() {
    let text = reduce(PAGE, MARKDOWN);
    for gone in [
        "color: red",
        "window.leak",
        "a comment",
        "Enable JS",
        "M0 0",
    ] {
        assert!(!text.contains(gone), "{gone:?} survived:\n{text}");
    }
}

#[test]
fn headings_keep_their_level_in_markdown_and_lose_it_in_plain() {
    assert!(reduce(PAGE, MARKDOWN).contains("# Perch & Rune"));
    assert!(!reduce(PAGE, PLAIN).contains('#'));
    assert!(reduce(PAGE, PLAIN).contains("Perch & Rune"));
}

#[test]
fn a_link_becomes_its_label_then_its_url_when_both_are_present() {
    let markdown = reduce(PAGE, MARKDOWN);
    assert!(
        markdown.contains("useful link (https://example.com/docs?a=1&b=2)"),
        "{markdown}"
    );
    assert!(!reduce(PAGE, PLAIN).contains("https://example.com"));
    assert!(reduce(PAGE, PLAIN).contains("useful link"));
}

#[test]
fn list_items_become_one_line_each() {
    let text = reduce("<ul><li>one</li><li>two</li></ul>", PLAIN);
    assert_eq!(text, "- one\n- two\n");
}

#[test]
fn lines_squeeze_and_blank_runs_collapse_to_one() {
    let text = reduce("<p>a   b</p><div></div><div></div><p>c\nd</p>", PLAIN);
    assert_eq!(text, "a b\n\nc d\n");
}

#[test]
fn entities_decode_and_unknown_ones_survive() {
    let text = reduce("<p>&lt;tag&gt; &amp; &#65; &#x42; &bogus; AT&T</p>", PLAIN);
    assert_eq!(text, "<tag> & A B &bogus; AT&T\n");
}

#[test]
fn an_anchor_without_a_label_falls_back_to_its_url() {
    assert_eq!(
        reduce("<a href=\"https://x.test\"></a>", MARKDOWN),
        "https://x.test\n"
    );
}

#[test]
fn an_unterminated_tag_does_not_loop_or_panic() {
    assert_eq!(reduce("<p>text <b", PLAIN), "text b\n");
    assert_eq!(reduce("a <script>never closed", PLAIN), "a\n");
}

#[test]
fn attribute_reads_both_quote_styles_and_refuses_a_longer_name() {
    assert_eq!(
        attribute(r#"a href="x" data-href="y""#, "href"),
        Some("x".to_string())
    );
    assert_eq!(attribute("a href='x'", "href"), Some("x".to_string()));
    assert_eq!(attribute("a data-href=\"y\"", "href"), None);
    assert_eq!(
        attribute("a href=https://x.test/z>t", "href"),
        Some("https://x.test/z".to_string())
    );
}

#[test]
fn empty_html_reduces_to_nothing() {
    assert_eq!(reduce("", MARKDOWN), "");
    assert_eq!(reduce("   \n\t ", MARKDOWN), "");
}
