//! Opening one card's whole output: which cards are open, and when they shut.
//!
//! A card is opened by its header or by the window's chord, and shut by Escape, by another turn
//! starting, or by the card's own result arriving — so an opened block is never left standing over
//! a card that has since changed underneath the reader.

use super::{Item, Transcript};

impl Transcript {
    /// Opens or closes the tool card `id`, answering whether it is open afterwards.
    ///
    /// An id that is not a card, and a card with nothing to read — a refusal, a call still running
    /// — is ignored: there is no empty block to open. One source for both the press and the key.
    pub fn toggle(&mut self, id: &str) -> bool {
        let readable = self.items.iter().rev().find_map(|item| match item {
            Item::Tool {
                id: card, output, ..
            } if card == id => Some(!output.trim().is_empty()),
            _ => None,
        });
        if readable != Some(true) {
            return false;
        }
        if let Some(index) = self.expanded.iter().position(|card| card == id) {
            self.expanded.remove(index);
            return false;
        }
        self.expanded.push(id.to_string());
        true
    }

    /// Shuts the card `id` without opening anything else, as its own result arriving does.
    pub fn collapse(&mut self, id: &str) {
        self.expanded.retain(|card| card != id);
    }

    /// Shuts every opened card, answering whether one was open. A new turn does this, and so does
    /// a turn ending: the column the reader was reading is not the one that continues.
    pub fn close_cards(&mut self) -> bool {
        let open = !self.expanded.is_empty();
        self.expanded.clear();
        open
    }

    /// Whether the tool card `id` is showing its whole output.
    #[must_use]
    pub fn is_expanded(&self, id: &str) -> bool {
        self.expanded.iter().any(|card| card == id)
    }

    /// The index of the newest tool card with an output to read, for the window's chord to open
    /// when no card holds the focus. The index is the item's place in the column, which is the
    /// `tool-<index>` key the column draws it under.
    #[must_use]
    pub fn last_card(&self) -> Option<usize> {
        self.items.iter().rposition(
            |item| matches!(item, Item::Tool { output, .. } if !output.trim().is_empty()),
        )
    }
}
