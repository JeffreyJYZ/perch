//! The transcript column: the conversation as items, and the view that draws them.
//!
//! [`Transcript`] is plain data — the agent's events in, items out — so the shell can own one per
//! tab and swap between them without reloading anything. [`transcript_view`] is the only drawing
//! code: assistant text goes through rune-kit's markdown view, in streaming mode while the turn
//! is still open, so half-written markup never flashes; a tool call is one card whose status is a
//! word, never a colour alone.
//!
//! The column scrolls as one: its header says how far above the newest turn it is ("scrolled 12
//! lines"), a reader scrolled up is never yanked to the bottom by arriving text, and the bottom
//! follows again once they return. Its plain turns share one selection group; a markdown reply
//! brings a group of its own from rune-kit, which is the one seam a drag cannot cross.

use perch_agent::AgentEvent;
use perch_tools::Preview;

mod card;
mod events;
mod item;
mod scroll;
mod turn;
mod view;
mod words;

pub use item::{Item, ToolStatus};
pub use view::{transcript_view, transcript_view_marked};
pub(crate) use words::scroll_words;
pub use words::{COLLAPSED_LINES, cap_line, collapsed_output, expanded_output, summary_line};

use scroll::ScrollWords;

/// One session's transcript: the items, plus which assistant item is still growing.
#[derive(Clone, Debug)]
pub struct Transcript {
    pub(super) items: Vec<Item>,
    /// Index of the assistant item still receiving deltas, if a turn is streaming into one.
    pub(super) open: Option<usize>,
    /// Ids of the tool cards showing their whole output; every other card is collapsed.
    pub(super) expanded: Vec<String>,
    /// Index of the first item of the turn being counted, for its closing summary.
    pub(super) since: usize,
    /// Assistant items whose turn was cancelled before they finished.
    pub(super) cancelled: Vec<usize>,
    /// Where the column says it is scrolled to, published to its header.
    scroll: ScrollWords,
    /// Draw with the dark palette. The shell sets this from the system; dark is the default.
    pub(super) dark: bool,
}

impl Default for Transcript {
    fn default() -> Transcript {
        Transcript {
            items: Vec::new(),
            open: None,
            expanded: Vec::new(),
            since: 0,
            cancelled: Vec::new(),
            scroll: ScrollWords::new(),
            dark: true,
        }
    }
}

impl Transcript {
    /// The items, in the order they were added.
    #[must_use]
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// Whether the dark palette is used for this transcript.
    #[must_use]
    pub fn dark(&self) -> bool {
        self.dark
    }

    /// Draws this transcript with the dark palette, or the light one.
    pub fn set_dark(&mut self, dark: bool) {
        self.dark = dark;
    }

    /// Where the column says it is scrolled to, bound to the header's text.
    pub(super) fn scroll(&self) -> &ScrollWords {
        &self.scroll
    }

    /// Whether the assistant item at `index` is still receiving deltas.
    pub(super) fn is_open(&self, index: usize) -> bool {
        self.open == Some(index)
    }

    /// Whether the assistant item at `index` belongs to a turn that was cancelled.
    pub(super) fn is_cancelled(&self, index: usize) -> bool {
        self.cancelled.contains(&index)
    }

    /// Adds a user turn and closes whatever was streaming: a new question starts a new answer.
    pub fn push_user(&mut self, text: impl Into<String>) {
        self.items.push(Item::User { text: text.into() });
        self.open = None;
        self.since = self.items.len();
    }

    /// Empties the column, as `/clean` asks: no item, no open reply, no card left expanded.
    ///
    /// The items are the session's whole conversation as the window shows it, so nothing is kept
    /// behind: a cleared column and a fresh one are the same column.
    pub fn clear(&mut self) {
        self.items.clear();
        self.open = None;
        self.expanded.clear();
        self.cancelled.clear();
        self.since = 0;
        self.scroll = ScrollWords::new();
    }

    /// Appends a system line — a cancellation, a provider error, a tool the user refused — in the
    /// column's quieter voice. It closes the open assistant item, because a system line is not
    /// part of a reply.
    pub fn note(&mut self, text: impl Into<String>) {
        self.open = None;
        self.items.push(Item::Note(text.into()));
    }

    /// Flips the tool card `id` between its collapsed lines and its whole output.
    ///
    /// An id that is not a card is ignored, so a click that outlived its card invents nothing.
    pub fn toggle(&mut self, id: &str) {
        if !self
            .items
            .iter()
            .any(|item| matches!(item, Item::Tool { id: card, .. } if card == id))
        {
            return;
        }
        if let Some(index) = self.expanded.iter().position(|card| card == id) {
            self.expanded.remove(index);
        } else {
            self.expanded.push(id.to_string());
        }
    }

    /// Whether the tool card `id` is showing its whole output.
    #[must_use]
    pub fn is_expanded(&self, id: &str) -> bool {
        self.expanded.iter().any(|card| card == id)
    }

    /// Marks the call `id` as waiting for the user, with the diff or command the approval shows.
    pub fn request_approval(&mut self, id: &str, preview: Preview) {
        if let Some(Item::Tool {
            status,
            preview: slot,
            ..
        }) = self.tool_mut(id)
        {
            *status = ToolStatus::ApprovalNeeded;
            *slot = Some(preview);
        }
    }

    /// Folds one agent event into the transcript.
    pub fn apply(&mut self, event: &AgentEvent) {
        events::apply(self, event);
    }

    /// The tool card whose id is `id`, when there is one.
    pub(super) fn tool_mut(&mut self, id: &str) -> Option<&mut Item> {
        self.items
            .iter_mut()
            .rev()
            .find(|item| matches!(item, Item::Tool { id: card, .. } if card == id))
    }
}
