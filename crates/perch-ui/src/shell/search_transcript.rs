//! ⌘F: the transcript's search bar, the hits it finds and the item each one is taken to.
//!
//! The scan lives in [`search_hits`] and runs over the transcript the shell already holds, so a
//! letter costs a scan of memory and never a disk read. The bar's field is state here rather than
//! in the transcript: the transcript is swapped on a session switch, while the search belongs to
//! the window and closes when it is told to.

use rune::{Key, KeyEvent, TextEdit, Tx};

use super::live::Live;
use super::search_hits::{self, Hit};
use super::state::Shell;

/// The prefix of the element a hit is drawn and scrolled to: `find-hit-<item>`.
pub const HIT: &str = "find-hit-";

/// The search's own state: the field, and which hit is showing.
#[derive(Default)]
pub(super) struct Find {
    pub(super) query: TextEdit,
    /// The hit the reader is on, clamped to the hits the field keeps when they are read.
    index: usize,
}

impl<L: Live> Shell<L> {
    /// Whether the transcript search bar is showing.
    pub fn find_open(&self) -> bool {
        self.find.is_some()
    }

    /// ⌘F: opens the bar, or closes it when it is already showing.
    ///
    /// The field takes the keyboard before the bar is mounted, the same way the file finder does:
    /// a field that must be clicked before typing reads as broken. The frame is committed
    /// instantly by the caller, so the focus lands on the first frame the bar is drawn.
    pub fn toggle_find(&mut self) {
        if self.find_open() {
            self.close_find();
            return;
        }
        let mut query = TextEdit::default();
        query.focused = true;
        self.find = Some(Find { query, index: 0 });
    }

    /// Hides the bar. The column is not scrolled or reloaded: it stays exactly where it was read.
    pub fn close_find(&mut self) {
        self.find = None;
    }

    /// What the bar's field holds, as typed.
    pub fn find_query_text(&self) -> String {
        self.find
            .as_ref()
            .map(|find| find.query.text.clone())
            .unwrap_or_default()
    }

    /// Every occurrence the query finds in the transcript, in reading order.
    pub fn find_hits(&self) -> Vec<Hit> {
        match self.find.as_ref() {
            Some(find) => search_hits::hits(self.transcript.items(), &find.query.text),
            None => Vec::new(),
        }
    }

    /// Which hit is showing, zero-based; `0` when there is nothing to show.
    pub fn find_index(&self) -> usize {
        let len = self.find_hits().len();
        match self.find.as_ref() {
            Some(find) if len > 0 => find.index.min(len - 1),
            _ => 0,
        }
    }

    /// The item the current hit lives in, which is what the column marks and scrolls to.
    pub fn find_mark(&self) -> Option<usize> {
        self.find_hits().get(self.find_index()).map(|hit| hit.item)
    }

    /// Moves to the next hit, wrapping at both ends.
    pub fn find_step(&mut self, step: isize) {
        let len = self.find_hits().len() as isize;
        if len <= 0 {
            return;
        }
        if let Some(find) = self.find.as_mut() {
            find.index = (find.index as isize + step).rem_euclid(len) as usize;
        }
    }

    /// The bar's words: `n of m`, `no matches`, or why there is nothing to search.
    pub fn find_words(&self) -> String {
        if !self.find_open() {
            return String::new();
        }
        if self.transcript.items().is_empty() {
            return "the transcript is empty".to_string();
        }
        if self.find_query_text().trim().is_empty() {
            return String::new();
        }
        let total = self.find_hits().len();
        if total == 0 {
            return "no matches".to_string();
        }
        format!("{} of {total}", self.find_index() + 1)
    }

    /// Takes the reader to the current hit: the item is scrolled into view and marked by the
    /// column. Called after the field changed and after Up or Down.
    pub fn find_scroll(&self, tx: &mut Tx) {
        if let Some(hit) = self.find_hits().get(self.find_index()) {
            tx.scroll_into_view(format!("{HIT}{}", hit.item));
        }
    }

    /// The field's own keys: Escape closes the bar, Up and Down move between hits and the column
    /// follows. Every other press is the field's, so a letter narrows the scan as it is typed.
    pub fn find_key(&mut self, ev: &KeyEvent, tx: &mut Tx) -> bool {
        if !self.find_open() || ev.mods.cmd || ev.mods.alt {
            return false;
        }
        match &ev.key {
            Key::Escape => {
                self.close_find();
                true
            }
            Key::Down => {
                self.find_step(1);
                tx.instant();
                self.find_scroll(tx);
                true
            }
            Key::Up => {
                self.find_step(-1);
                tx.instant();
                self.find_scroll(tx);
                true
            }
            _ => false,
        }
    }

    /// After the field changed: the showing hit stays in range and the column moves to it.
    pub fn find_changed(&mut self, tx: &mut Tx) {
        let len = self.find_hits().len();
        if let Some(find) = self.find.as_mut() {
            find.index = find.index.min(len.saturating_sub(1));
        }
        tx.instant();
        self.find_scroll(tx);
    }
}

/// A bare Escape anywhere else in the window, with the bar open, closes it.
///
/// Attached to the window's root, after every region that owns Escape has had it: an overlay
/// closes first, and only a press that would otherwise do nothing shuts the bar.
pub(super) fn escape_key<L: Live>(shell: &mut Shell<L>, ev: &KeyEvent) -> bool {
    if !shell.find_open() || ev.mods.cmd || ev.mods.alt || !matches!(&ev.key, Key::Escape) {
        return false;
    }
    shell.close_find();
    true
}
