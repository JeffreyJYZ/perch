//! A transcript card's keys: Enter or Space on the focused card opens or closes it, ⌘O opens the
//! one with the focus, and Escape shuts whatever is open.
//!
//! The column draws one card per tool call and the card is focusable, so the press that opens it
//! needs nothing more than which card holds the focus. Rune reports that on [`Tx::focused`], which
//! is why this is read where the window's keys are — the card itself has no handler of its own
//! beyond the press on its header.

use rune::{Key, KeyEvent, Tx};

use super::composer::composer_key;
use super::live::Live;
use super::state::Shell;
use crate::preview::preview_key;
use crate::transcript::{CardPress, Item, Transcript};

/// Whether `ev` is the press that opens or closes a card: Enter or Space, with nothing held.
pub fn toggles_card(ev: &KeyEvent) -> bool {
    if ev.mods.cmd || ev.mods.ctrl || ev.mods.alt || ev.mods.logo {
        return false;
    }
    match &ev.key {
        Key::Enter => true,
        Key::Text(text) => text == " ",
        _ => false,
    }
}

impl<L: Live> CardPress for Shell<L> {
    /// A press on a card's header, which opens the card in the app's transcript and in the copy the
    /// window draws, so the frame after the press is already right.
    fn card_press(&mut self, index: usize, _id: &str) {
        self.toggle_card(&format!("tool-{index}"));
    }
}

impl<L: Live> Shell<L> {
    /// Flips the tool card keyed `key`, which is the `tool-<index>` the column draws — or the
    /// `tool-<index>-head` its header carries, since a press on the header takes the focus.
    ///
    /// The app's transcript is the one toggled, because the shell holds a copy: toggling the copy
    /// alone would last only until the next frame replaced it. The copy is toggled too, so the
    /// frame after this press is already right. A card with nothing to read answers false.
    pub fn toggle_card(&mut self, key: &str) -> bool {
        let Some(index) = card_index(key) else {
            return false;
        };
        let Some(Item::Tool { id, output, .. }) = self.transcript.items().get(index) else {
            return false;
        };
        if output.trim().is_empty() {
            return false;
        }
        let id = id.clone();
        if let Some(transcript) = self.live.as_mut().and_then(Live::transcript_mut) {
            transcript.toggle(&id);
        }
        self.transcript.toggle(&id);
        true
    }

    /// Opens the focused card's output, or the newest card's when no card holds the focus: the
    /// chord is the keyboard's way in, so it never depends on a press having come first.
    pub fn open_card(&mut self, tx: &Tx) -> bool {
        let focused = tx.focused().map(str::to_owned);
        self.open_card_at(focused.as_deref())
    }

    /// [`Shell::open_card`] with the focused key named, which is what a test can say without a
    /// running window.
    pub fn open_card_at(&mut self, focused: Option<&str>) -> bool {
        let key = match focused {
            Some(card) if card_index(card).is_some() => card.to_string(),
            _ => match self.transcript.last_card() {
                Some(index) => format!("tool-{index}"),
                None => return false,
            },
        };
        self.toggle_card(&key)
    }

    /// Shuts every opened card, answering whether one was open. Escape asks this, and so does the
    /// turn continuing: see [`Transcript::close_cards`].
    pub fn close_cards(&mut self) -> bool {
        let app = self
            .live
            .as_mut()
            .and_then(Live::transcript_mut)
            .is_some_and(Transcript::close_cards);
        app | self.transcript.close_cards()
    }
}

/// The item index a card key names: `tool-3` and `tool-3-head` are both the fourth item.
fn card_index(key: &str) -> Option<usize> {
    key.strip_prefix("tool-")?
        .split('-')
        .next()?
        .parse::<usize>()
        .ok()
}

/// The focused card's key: Enter or Space opens or closes it, and nothing else is touched.
///
/// This is attached to the window's root, because Rune runs an element's `on_key` handlers —
/// the focused element's and its ancestors' — before a focused control's own activation. Space
/// is that activation key, and a card has no press handler, so the root is where Space can be
/// answered. A press that is not this one falls through to the rest of the window untouched.
pub fn card_key<L: Live>(shell: &mut Shell<L>, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let focused = tx.focused().map(str::to_owned);
    let Some(card) = focused.as_deref() else {
        return false;
    };
    toggles_card(ev) && shell.toggle_card(card)
}

/// Escape on an opened card: a bare Escape shuts it and is answered, and every other press falls
/// through. It runs after the search bars, so a bar that is open takes Escape first, and after the
/// composer, so an approval or a question keeps the Escape it answers itself.
pub fn card_escape<L: Live>(shell: &mut Shell<L>, ev: &KeyEvent) -> bool {
    if ev.mods.cmd || ev.mods.ctrl || ev.mods.alt || ev.mods.logo {
        return false;
    }
    matches!(ev.key, Key::Escape) && shell.close_cards()
}

/// Every key the window itself answers: a focused card first, then the preview's Escape, then the
/// composer's map, the two Escapes that clear a filter or shut a search bar, and last the Escape
/// that shuts an opened card — so an overlay that owns Escape still closes first.
///
/// The composer's own return would break the line, so a card that holds the focus takes Return
/// before the field sees it; with nothing focused this is exactly [`composer_key`] after
/// [`preview_key`], which only claims a bare Escape.
pub fn window_key<L: Live>(shell: &mut Shell<L>, ev: &KeyEvent, tx: &mut Tx) -> bool {
    card_key(shell, ev, tx)
        || preview_key(shell, ev)
        || composer_key(shell, ev, tx)
        || super::search_transcript::escape_key(shell, ev)
        || super::search_session::escape_key(shell, ev)
        || card_escape(shell, ev)
}

#[cfg(test)]
mod tests;
