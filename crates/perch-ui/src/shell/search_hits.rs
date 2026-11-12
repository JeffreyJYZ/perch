//! Every place a transcript search finds its needle.
//!
//! The words themselves are [`search_visible`](super::search_visible)'s job: this module counts
//! occurrences in the text that pass returns, so the count is over what the column draws and never
//! over markdown source. The scan is over the transcript held in memory, which is why typing stays
//! responsive — there is no file, no walk and no app call between a letter and the count.

use crate::transcript::Item;

pub use super::search_visible::{visible_needle, visible_text};

/// One occurrence a search found, and the item that holds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    /// The index of the transcript item whose visible text holds the match.
    pub item: usize,
}

/// Every occurrence of `needle` in `items`, in reading order.
///
/// Occurrences inside one item are counted one by one, so `n of m` says how many times the word is
/// there rather than how many turns mention it. A needle that is empty once its markers are gone
/// matches nothing: an empty search asks for no hits, not for every item.
#[must_use]
pub fn hits(items: &[Item], needle: &str) -> Vec<Hit> {
    let needle = visible_needle(needle);
    if needle.is_empty() {
        return Vec::new();
    }
    let mut found = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let text = visible_text(item).to_lowercase();
        let mut from = 0;
        while let Some(at) = text[from..].find(&needle) {
            found.push(Hit { item: index });
            from += at + needle.len();
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::hits;
    use crate::transcript::Item;

    fn assistant(text: &str) -> Item {
        Item::Assistant {
            text: text.to_string(),
        }
    }

    fn tool(output: &str) -> Item {
        Item::Tool {
            id: "call-1".to_string(),
            name: "run".to_string(),
            status: crate::transcript::ToolStatus::Ok,
            output: output.to_string(),
            preview: None,
            target: Some("src/main.rs".to_string()),
        }
    }

    #[test]
    fn a_heading_is_found_by_its_words_with_or_without_the_hashes() {
        let items = [assistant("## Build and test\n\nrun it twice")];
        assert_eq!(hits(&items, "Heading"), Vec::new());
        assert_eq!(hits(&items, "Build").len(), 1);
        assert_eq!(
            hits(&items, "## Build").len(),
            1,
            "a heading searched for with its hashes still finds the words"
        );
        assert!(
            hits(&items, "##").is_empty(),
            "markers alone are not words to find"
        );
    }

    #[test]
    fn every_occurrence_in_the_visible_text_is_a_hit() {
        let items = [
            Item::User {
                text: "the build is red".to_string(),
            },
            assistant("the build failed, so the build is red"),
            tool("line 1\nline 9: build error"),
            Item::Note("a note".to_string()),
        ];
        let found = hits(&items, "BUILD");
        assert_eq!(
            found.iter().map(|hit| hit.item).collect::<Vec<_>>(),
            [0, 1, 1, 2],
            "case is ignored and one item can hold several hits"
        );
        assert_eq!(
            hits(&items, "main.rs")
                .iter()
                .map(|hit| hit.item)
                .collect::<Vec<_>>(),
            [2],
            "a card's header names its file and its output its lines"
        );
        assert!(hits(&items, "   ").is_empty(), "spaces are not a search");
    }
}
