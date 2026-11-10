//! Where the column is scrolled to, in the words its header shows.
//!
//! The offset leaves Rune as a point measurement from the newest turn, and a reader needs lines,
//! so the two are turned into words here. The words are published to a [`LiveText`] rather than
//! held as state: `El::on_scroll_offset` runs inside the frame that scrolled and may not change
//! state, so the header element is told directly and shows the new words in the same frame.

use rune::{LiveText, ScrollPos};

use super::words::scroll_words;

/// The line height to count lines by when the caller has no body font at hand.
const FALLBACK_LINE: f32 = 20.0;

/// The words over how far above the newest turn the column is scrolled, bound to the header.
#[derive(Clone, Debug)]
pub(crate) struct ScrollWords(LiveText);

impl ScrollWords {
    /// The header's words, starting with none: a column that has not scrolled says nothing.
    pub(crate) fn new() -> ScrollWords {
        ScrollWords(LiveText::new(""))
    }

    /// The text the header binds, so the words arrive without a view run.
    pub(crate) fn text(&self) -> &LiveText {
        &self.0
    }

    /// Records where the viewport is, in the words the header shows.
    pub(crate) fn track(&self, at: ScrollPos, line: f32) {
        self.0.set(up_words(&at, line).unwrap_or_default());
    }
}

/// How far above the newest turn `at` is, in the words a reader needs; `None` at the bottom.
///
/// The count is in body lines, so it says how much reading the reader skipped rather than a
/// number of points. A column that fits its viewport has no distance to report.
fn up_words(at: &ScrollPos, line: f32) -> Option<String> {
    let up = (at.max[1] - at.offset[1]).max(0.0);
    if up < 1.0 {
        return None;
    }
    let line = if line > 0.0 { line } else { FALLBACK_LINE };
    let lines = (up / line).round().max(1.0) as usize;
    scroll_words(lines)
}

#[cfg(test)]
mod tests {
    use rune::ScrollPos;

    use super::{ScrollWords, up_words};

    fn pos(offset: f32, max: f32) -> ScrollPos {
        // `ScrollPos` is non-exhaustive, so it is built from its default and then positioned.
        let mut at = ScrollPos::default();
        at.offset = [0.0, offset];
        at.max = [0.0, max];
        at.viewport = [900.0, 500.0];
        at.drawn = [900.0, 500.0];
        at
    }

    #[test]
    fn the_distance_from_the_bottom_is_counted_in_body_lines() {
        assert_eq!(
            up_words(&pos(0.0, 0.0), 20.0),
            None,
            "a column that fits says nothing"
        );
        assert_eq!(
            up_words(&pos(400.0, 600.0), 20.0).as_deref(),
            Some("scrolled 10 lines"),
            "200 points above the bottom at 20 a line"
        );
        assert_eq!(
            up_words(&pos(590.0, 600.0), 20.0).as_deref(),
            Some("scrolled 1 line"),
            "a nudge is never claimed as nothing"
        );
        assert_eq!(
            up_words(&pos(100.0, 300.0), 0.0).as_deref(),
            Some("scrolled 10 lines"),
            "no line height falls back to a body line"
        );
    }

    #[test]
    fn the_header_says_nothing_at_the_bottom_and_says_the_distance_above_it() {
        let words = ScrollWords::new();
        assert_eq!(&*words.text().get(), "", "a fresh column is at the bottom");
        words.track(pos(300.0, 300.0), 20.0);
        assert_eq!(&*words.text().get(), "", "the bottom still says nothing");
        words.track(pos(100.0, 300.0), 20.0);
        assert_eq!(&*words.text().get(), "scrolled 10 lines");
    }
}
